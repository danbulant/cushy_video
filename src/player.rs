use std::{
    sync::{atomic::Ordering, Arc},
    time::Instant,
};

use cushy::{
    context::{GraphicsContext, LayoutContext},
    figures::{units::UPx, Size},
    value::{Destination, Dynamic, DynamicReader, Generation, Source},
    widget::Widget,
    ConstraintLimit,
};

use crate::{
    pipeline::{VideoPrimitive, VideoRO},
    video::Video,
    Error,
};

/// A video player widget, with no builtin controls.
/// Autoplays by default.
/// Supports subtitles, but doesn't render them - see [VideoPlayer::get_subtitles]
#[derive(Debug)]
pub struct VideoPlayer {
    video: Video,
    subtitles: Dynamic<Option<String>>,
    frame: Dynamic<()>,
    last_frame: Generation,
}

impl VideoPlayer {
    pub fn new(video: Video) -> Self {
        let subtitles = video.0.read().unwrap().subtitles.clone();
        let frame = video.0.read().unwrap().upload_frame.clone();
        Self {
            subtitles,
            video,
            last_frame: Generation::default(),
            frame,
        }
    }

    pub fn from_url(url: &url::Url) -> Result<Self, Error> {
        Ok(Self::new(Video::new(url)?))
    }

    /// Returns a dynamic source that can be used to get the subtitles, if present.
    /// Currently, HTML entities are unescaped, but no other processing is done. No rich text support.
    #[must_use]
    pub fn get_subtitles(&self) -> DynamicReader<Option<String>> {
        self.subtitles.clone().into_reader()
    }

    /// Gets a dynamic reader that can be used to listen to frame changes.
    #[must_use]
    pub fn on_frame(&self) -> DynamicReader<()> {
        self.frame.clone().into_reader()
    }

    /// Gets a read handle on the inner Video object. Can be used to control playback and get metadata.
    pub fn video(&self) -> &Video {
        &self.video
    }
}

impl Widget for VideoPlayer {
    fn redraw(&mut self, context: &mut GraphicsContext<'_, '_, '_, '_>) {
        let mut inner = self.video.write();
        let frame = inner.upload_frame.generation();
        let _ = inner.upload_frame.get_tracking_redraw(context); // no data here, just to trigger redraw

        let upload_frame = frame != self.last_frame;
        self.last_frame = frame;

        if upload_frame {
            let last_frame_time = inner
                .last_frame_time
                .lock()
                .map(|time| *time)
                .unwrap_or_else(|_| Instant::now());
            inner.set_av_offset(Instant::now() - last_frame_time);
        }

        context.gfx.draw_with::<VideoRO>(VideoPrimitive::new(
            inner.id,
            Arc::clone(&inner.alive),
            Arc::clone(&inner.frame),
            (inner.width as _, inner.height as _),
            upload_frame,
        ));
    }

    fn layout(
        &mut self,
        available_space: Size<ConstraintLimit>,
        _context: &mut LayoutContext<'_, '_, '_, '_>,
    ) -> Size<UPx> {
        available_space.map(ConstraintLimit::max)
    }
}
