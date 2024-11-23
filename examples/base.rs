use cushy::{widget::MakeWidget, Run};
use cushy_video::{player::VideoPlayer, video::Video};

fn main() -> cushy::Result {
    VideoPlayer::new(
        Video::new(
            &url::Url::from_file_path(
                std::path::PathBuf::from(file!())
                    .parent()
                    .unwrap()
                    .join("../media/big-buck-bunny-480p-30sec.mp4")
                    .canonicalize()
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .contain()
    .pad()
    .expand()
    .run()
}
