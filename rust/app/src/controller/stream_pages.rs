use feishin_core::model::Song;

use super::list_streams::{ActiveStream, PageSide};
use super::{Controller, UiUpdate};
use crate::list_stream::{Placement, StreamEvent, StreamId, StreamMessage, StreamPage};

impl Controller {
    pub(super) fn on_list_stream(&mut self, message: StreamMessage) {
        let StreamMessage { stream, event } = message;
        let Some(placement) = self
            .list_streams
            .get_mut(stream)
            .map(|active| active.placement())
        else {
            tracing::debug!("dropping an update of a replaced list stream");
            return;
        };
        match event {
            StreamEvent::Start { page, start_index } => {
                self.apply_stream_start(stream, placement, page, start_index);
            }
            StreamEvent::After(page) => self.apply_stream_page(stream, page, PageSide::After),
            StreamEvent::Before(page) => self.apply_stream_page(stream, page, PageSide::Before),
            StreamEvent::Failed(error) => self.list_stream_failed(stream, &error),
            StreamEvent::Finished => self.list_stream_finished(stream),
        }
    }

    fn apply_stream_start(
        &mut self,
        stream: StreamId,
        placement: Placement,
        page: StreamPage,
        start_index: usize,
    ) {
        match placement {
            Placement::Now => {
                let StreamPage { songs, applied } = page;
                self.start_streamed_playback(stream, songs, start_index);
                applied.confirm();
            }
            Placement::Next | Placement::Last => {
                self.apply_stream_page(stream, page, PageSide::After);
            }
        }
    }

    fn start_streamed_playback(&mut self, stream: StreamId, songs: Vec<Song>, start_index: usize) {
        if songs.is_empty() {
            self.list_streams.remove(stream);
            self.deps.ui.send(UiUpdate::Error(
                "There is nothing to play in this list".into(),
            ));
            return;
        }
        self.queue.play_now_at(songs, Some(start_index));
        let first_uid = self.queue.items().first().map_or(0, |item| item.uid);
        let end_uid = self.queue.items().last().map_or(0, |item| item.uid + 1);
        if let Some(active) = self.list_streams.get_mut(stream) {
            active.record_start(first_uid..end_uid);
        }
        self.publish_queue();
        self.load_current(0);
    }

    fn apply_stream_page(&mut self, stream: StreamId, page: StreamPage, side: PageSide) {
        let StreamPage { songs, applied } = page;
        if let Some(active) = self.list_streams.get_mut(stream) {
            let anchor = active.anchor(side, &self.queue);
            let inserted = match active.placement() {
                Placement::Now => self
                    .queue
                    .insert_interleaved(&songs, anchor, |uid| active.owns(uid)),
                Placement::Next | Placement::Last => self.queue.insert_at_anchor(&songs, anchor),
            };
            active.record(side, inserted);
        }
        applied.confirm();
        self.after_stream_page(stream);
    }

    fn after_stream_page(&mut self, stream: StreamId) {
        self.refresh_preload();
        let publish_due = self
            .list_streams
            .get_mut(stream)
            .is_some_and(ActiveStream::publish_due);
        if publish_due {
            self.publish_queue();
        }
    }

    fn list_stream_failed(&mut self, stream: StreamId, error: &str) {
        let Some(active) = self.list_streams.remove(stream) else {
            return;
        };
        let message = match active.is_started() {
            true => {
                format!("The queue is incomplete: loading the rest of the list failed ({error})")
            }
            false => format!("Could not load the list: {error}"),
        };
        self.deps.ui.send(UiUpdate::Error(message));
        if active.is_started() {
            self.finish_stream_changes();
        }
    }

    fn list_stream_finished(&mut self, stream: StreamId) {
        let started = self
            .list_streams
            .remove(stream)
            .is_some_and(|active| active.is_started());
        if started {
            self.finish_stream_changes();
        }
    }

    fn finish_stream_changes(&mut self) {
        self.publish_queue();
        self.refresh_preload();
        self.schedule_queue_sync();
    }

    #[cfg(test)]
    pub fn is_streaming(&self) -> bool {
        !self.list_streams.is_empty()
    }
}
