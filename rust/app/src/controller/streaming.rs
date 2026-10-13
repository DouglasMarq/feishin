use std::ops::Range;

use super::list_streams::ActiveStream;
use super::{Command, Controller, forward_command};
use crate::list_stream::{
    Deliver, Delivery, ListRow, Placement, RetryPolicy, RowRanges, ServerPages, SongSource,
    StreamRunner, StreamStart,
};

impl Controller {
    pub(super) fn dispatch_list_command(&mut self, command: Command) {
        match command {
            Command::PlayFromList {
                source,
                song_id,
                row_index,
            } => self.play_from_list(source, song_id, row_index),
            Command::EnqueueFromList {
                source,
                ranges,
                placement,
            } => self.enqueue_from_list(source, ranges, placement),
            Command::ListStream(message) => self.on_list_stream(message),
            _ => tracing::debug!("ignoring a command that does not stream a list"),
        }
    }

    fn play_from_list(&mut self, source: SongSource, song_id: Option<String>, row_index: ListRow) {
        let start = StreamStart::Row {
            row: row_index,
            song_id,
        };
        self.start_list_stream(source, RowRanges::whole_list(), Placement::Now, start);
    }

    fn enqueue_from_list(
        &mut self,
        source: SongSource,
        ranges: Vec<Range<usize>>,
        placement: Placement,
    ) {
        let start = match (placement, self.queue.shuffle()) {
            (Placement::Now, true) => StreamStart::RandomRow,
            _ => StreamStart::FirstRow,
        };
        self.start_list_stream(source, RowRanges::from_ranges(ranges), placement, start);
    }

    fn start_list_stream(
        &mut self,
        source: SongSource,
        listed: RowRanges,
        placement: Placement,
        start: StreamStart,
    ) {
        if placement == Placement::Now {
            self.play_request += 1;
            self.list_streams.cancel_all();
        }
        let Some(server) = self.deps.session.get() else {
            return;
        };
        let stream = self.list_streams.next_id();
        let commands = self.deps.commands.clone();
        let deliver: Deliver = Box::new(move |message| {
            forward_command(&commands, Command::ListStream(message));
        });
        let fetcher = Box::new(ServerPages { server, source });
        let delivery = Delivery::new(stream, deliver);
        let runner = StreamRunner::new(fetcher, listed, RetryPolicy::PATIENT, delivery);
        let task = tokio::spawn(runner.run(start)).abort_handle();
        self.list_streams
            .insert(stream, ActiveStream::new(placement, task));
    }
}
