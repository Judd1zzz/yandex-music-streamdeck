use ym_model::VolumeAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalCommand {
    PlayPause,
    Next,
    Prev,
    ToggleLike,
    ToggleDislike,
    ToggleShuffle,
    ToggleRepeat,
    ChangeVolume(VolumeAction),
}

impl LocalCommand {
    pub fn kind(self) -> &'static str {
        match self {
            LocalCommand::PlayPause => "playPause",
            LocalCommand::Next => "next",
            LocalCommand::Prev => "prev",
            LocalCommand::ToggleLike => "toggleLike",
            LocalCommand::ToggleDislike => "toggleDislike",
            LocalCommand::ToggleShuffle => "toggleShuffle",
            LocalCommand::ToggleRepeat => "toggleRepeat",
            LocalCommand::ChangeVolume(_) => "changeVolume",
        }
    }

    pub fn expr(self, ctrl: &str) -> String {
        match self {
            LocalCommand::PlayPause => format!("{ctrl}.playPause()"),
            LocalCommand::Next => format!("{ctrl}.next()"),
            LocalCommand::Prev => format!("{ctrl}.prev()"),
            LocalCommand::ToggleLike => format!("{ctrl}.toggleLike()"),
            LocalCommand::ToggleDislike => format!("{ctrl}.toggleDislike()"),
            LocalCommand::ToggleShuffle => format!("{ctrl}.toggleShuffle()"),
            LocalCommand::ToggleRepeat => format!("{ctrl}.toggleRepeat()"),
            LocalCommand::ChangeVolume(action) => {
                let (name, value) = match action {
                    VolumeAction::Up => ("UP", 0u8),
                    VolumeAction::Down => ("DOWN", 0),
                    VolumeAction::Set(p) => ("SET", p),
                    VolumeAction::Mute => ("MUTE", 0),
                };
                format!("{ctrl}.changeVolume('{name}', {value})")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expr_strings() {
        let c = "window[\"_h\"]";
        assert_eq!(LocalCommand::PlayPause.expr(c), "window[\"_h\"].playPause()");
        assert_eq!(LocalCommand::Next.expr(c), "window[\"_h\"].next()");
        assert_eq!(LocalCommand::Prev.expr(c), "window[\"_h\"].prev()");
        assert_eq!(LocalCommand::ToggleLike.expr(c), "window[\"_h\"].toggleLike()");
        assert_eq!(LocalCommand::ToggleDislike.expr(c), "window[\"_h\"].toggleDislike()");
        assert_eq!(LocalCommand::ToggleShuffle.expr(c), "window[\"_h\"].toggleShuffle()");
        assert_eq!(LocalCommand::ToggleRepeat.expr(c), "window[\"_h\"].toggleRepeat()");
        assert_eq!(
            LocalCommand::ChangeVolume(VolumeAction::Set(35)).expr(c),
            "window[\"_h\"].changeVolume('SET', 35)"
        );
        assert_eq!(
            LocalCommand::ChangeVolume(VolumeAction::Up).expr(c),
            "window[\"_h\"].changeVolume('UP', 0)"
        );
        assert_eq!(
            LocalCommand::ChangeVolume(VolumeAction::Down).expr(c),
            "window[\"_h\"].changeVolume('DOWN', 0)"
        );
        assert_eq!(
            LocalCommand::ChangeVolume(VolumeAction::Mute).expr(c),
            "window[\"_h\"].changeVolume('MUTE', 0)"
        );
    }

    #[test]
    fn kind_strings() {
        assert_eq!(LocalCommand::PlayPause.kind(), "playPause");
        assert_eq!(LocalCommand::Next.kind(), "next");
        assert_eq!(LocalCommand::Prev.kind(), "prev");
        assert_eq!(LocalCommand::ToggleLike.kind(), "toggleLike");
        assert_eq!(LocalCommand::ToggleDislike.kind(), "toggleDislike");
        assert_eq!(LocalCommand::ToggleShuffle.kind(), "toggleShuffle");
        assert_eq!(LocalCommand::ToggleRepeat.kind(), "toggleRepeat");
        assert_eq!(LocalCommand::ChangeVolume(VolumeAction::Mute).kind(), "changeVolume");
    }
}
