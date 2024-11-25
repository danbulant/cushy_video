use std::{
    sync::{atomic::Ordering, Arc},
    time::Instant,
};

use cushy::{
    context::{GraphicsContext, LayoutContext},
    figures::{
        units::{Px, UPx},
        FloatConversion, IntoSigned, IntoUnsigned, Point, Rect, Size,
    },
    value::{Destination, Dynamic, DynamicReader, Generation, Source, Value},
    widget::Widget,
    widgets::image::{Aspect, ImageScaling},
    ConstraintLimit,
};

use crate::{
    pipeline::{VideoPrimitive, VideoRO},
    video::{Internal, Video},
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
    scaling: Value<ImageScaling>,
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
            scaling: Default::default(),
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

    fn calculate_video_rect(
        &self,
        video: &Internal,
        within_size: Size<UPx>,
        context: &mut GraphicsContext<'_, '_, '_, '_>,
    ) -> Rect<Px> {
        let within_size = within_size.into_signed();
        let size = Size {
            width: Px::new(video.width),
            height: Px::new(video.height),
        };
        match self.scaling.get_tracking_invalidate(context) {
            ImageScaling::Aspect { mode, orientation } => {
                let scale_width = within_size.width.into_float() / size.width.into_float();
                let scale_height = within_size.height.into_float() / size.height.into_float();

                let effective_scale = match mode {
                    Aspect::Fill => scale_width.max(scale_height),
                    Aspect::Fit => scale_width.min(scale_height),
                };
                let scaled = size * effective_scale;

                let x = (within_size.width - scaled.width) * *orientation.width;
                let y = (within_size.height - scaled.height) * *orientation.height;

                Rect::new(Point::new(x, y), scaled)
            }
            ImageScaling::Stretch => within_size.into(),
            ImageScaling::Scale(factor) => {
                let size = size.map(|px| px * factor);
                size.into()
            }
        }
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
        context: &mut LayoutContext<'_, '_, '_, '_>,
    ) -> Size<UPx> {
        let inner = self.video.read();
        let rect =
            self.calculate_video_rect(&inner, available_space.map(ConstraintLimit::max), context);
        rect.size.into_unsigned()
    }
}
