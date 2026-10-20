use std::sync::Mutex;

use async_trait::async_trait;
use feishin_core::queue::{Queue, QueueTrack, YouTubeTrack};
use feishin_remote::{HostDelegate, RemoteControl, ServerKey};

use super::*;
use crate::remote::approval::Approvals;
use crate::store::JsonStore;

struct SpeakerDelegate {
    adopted: Mutex<Vec<TransferPayload>>,
}

#[async_trait]
impl HostDelegate for SpeakerDelegate {
    async fn approve(&self, _device: DeviceInfo) -> bool {
        true
    }
    fn control(&self, _device: &DeviceInfo, _control: RemoteControl) -> Result<(), RemoteFailure> {
        Ok(())
    }
    async fn adopt(&self, payload: TransferPayload) -> Result<(), RemoteFailure> {
        self.adopted.lock().unwrap().push(payload);
        Ok(())
    }
    async fn hand_over(&self) -> Result<TransferPayload, RemoteFailure> {
        Ok(payload_of(2, 7_000))
    }
    fn server(&self) -> Option<ServerKey> {
        None
    }
}

#[derive(Default)]
struct Shown(Mutex<Vec<crate::controller::UiUpdate>>);

impl UiSink for Shown {
    fn send(&self, update: crate::controller::UiUpdate) {
        self.0.lock().unwrap().push(update);
    }
}

fn payload_of(count: usize, position_ms: u64) -> TransferPayload {
    let mut queue = Queue::with_seed(5);
    let tracks: Vec<QueueTrack> = (0..count)
        .map(|index| {
            QueueTrack::from(&YouTubeTrack {
                video_id: format!("video{index}"),
                title: format!("Video {index}"),
                artists: Vec::new(),
                album: None,
                thumbnail_url: None,
                duration_ms: 100_000,
            })
        })
        .collect();
    queue.play_tracks_now(tracks, Some(0));
    TransferPayload {
        snapshot: queue.snapshot(),
        position_ms,
        playing: true,
        server: None,
    }
}

fn device(id: &str) -> DeviceInfo {
    DeviceInfo {
        id: DeviceId(id.into()),
        name: id.into(),
    }
}

struct Setup {
    service: RemoteService,
    commands: mpsc::UnboundedReceiver<Command>,
    speaker: Arc<SpeakerDelegate>,
    shown: Arc<Shown>,
    target: PlayerTarget,
    _store: tempfile::TempDir,
}

struct Speaker {
    delegate: Arc<SpeakerDelegate>,
    address: std::net::SocketAddr,
    state: watch::Receiver<HostState>,
    store: Arc<dyn crate::store::Store>,
    store_dir: tempfile::TempDir,
}

async fn start_speaker() -> Speaker {
    let delegate = Arc::new(SpeakerDelegate {
        adopted: Mutex::new(Vec::new()),
    });
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let store_dir = tempfile::tempdir().unwrap();
    let store: Arc<dyn crate::store::Store> = Arc::new(JsonStore::new(store_dir.path()));
    let (_state, state) = watch::channel(HostState::default());
    let trust = Arc::new(FileTrust::ephemeral(Arc::clone(&store)));
    let host = Host::new(device("speaker"), delegate.clone(), trust, state.clone());
    tokio::spawn(host.serve(listener));
    Speaker {
        delegate,
        address,
        state,
        store,
        store_dir,
    }
}

fn client_deps(
    speaker: &Speaker,
    commands: mpsc::UnboundedSender<Command>,
    mirror: Arc<Shown>,
    target: PlayerTarget,
) -> ServiceDeps {
    ServiceDeps {
        commands: commands.clone(),
        session: Session::default(),
        window: slint::Weak::default(),
        trust: Arc::new(FileTrust::ephemeral(Arc::clone(&speaker.store))),
        me: device("laptop"),
        target,
        mirror,
        host_state: speaker.state.clone(),
        delegate: Arc::new(AppHostDelegate {
            commands,
            session: Session::default(),
            approvals: Approvals::default(),
            window: slint::Weak::default(),
        }),
    }
}

async fn setup() -> Setup {
    let speaker = start_speaker().await;
    let (commands_sender, commands) = mpsc::unbounded_channel();
    let shown = Arc::new(Shown::default());
    let target = PlayerTarget::default();
    let service = RemoteService {
        deps: client_deps(&speaker, commands_sender, shown.clone(), target.clone()),
        discovery: None,
        found: vec![Discovered {
            device: device("speaker"),
            address: speaker.address,
        }],
        hosting: None,
        link: None,
    };
    Setup {
        service,
        commands,
        speaker: speaker.delegate,
        shown,
        target,
        _store: speaker.store_dir,
    }
}

async fn next_command(commands: &mut mpsc::UnboundedReceiver<Command>) -> Command {
    tokio::time::timeout(Duration::from_secs(5), commands.recv())
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn controlling_a_device_pauses_this_one_and_shows_the_other() {
    let mut setup = setup().await;
    assert!(setup.service.control(&DeviceId("speaker".into())).await);
    assert!(setup.target.is_remote());
    assert!(matches!(
        next_command(&mut setup.commands).await,
        Command::Media(MediaControlEvent::Pause)
    ));
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert!(
        !setup.shown.0.lock().unwrap().is_empty(),
        "the remote state is shown"
    );
    setup.service.release_link(true);
    assert!(!setup.target.is_remote());
    assert!(matches!(
        next_command(&mut setup.commands).await,
        Command::Remote(RemoteCommand::Republish)
    ));
}

#[tokio::test]
async fn moving_the_music_here_adopts_the_other_queue() {
    let mut setup = setup().await;
    assert!(setup.service.control(&DeviceId("speaker".into())).await);
    next_command(&mut setup.commands).await;
    setup.service.move_here().await;
    let Command::Remote(RemoteCommand::AdoptTransfer(adopted)) =
        next_command(&mut setup.commands).await
    else {
        panic!("expected the queue to be adopted");
    };
    assert_eq!(adopted.snapshot.items.len(), 2);
    assert_eq!((adopted.position_ms, adopted.playing), (7_000, true));
    assert!(!setup.target.is_remote());
}

#[tokio::test]
async fn moving_the_music_there_hands_this_queue_over() {
    let mut setup = setup().await;
    let mut commands = setup.commands;
    let answering = tokio::spawn(async move {
        while let Some(command) = commands.recv().await {
            if let Command::Remote(RemoteCommand::TakeTransfer(reply)) = command {
                reply.send(Ok(payload_of(3, 12_000))).ok();
            }
        }
    });
    setup.service.move_to(&DeviceId("speaker".into())).await;
    let adopted = setup.speaker.adopted.lock().unwrap().clone();
    assert_eq!(adopted.len(), 1);
    assert_eq!(
        adopted.first().map(|payload| payload.position_ms),
        Some(12_000)
    );
    assert!(setup.target.is_remote());
    answering.abort();
}
