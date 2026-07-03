use ym_model::VolumeAction;

use crate::JS_CONTROLLER_NAME;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalCommand {
    PlayPause,
    Next,
    Prev,
    ToggleLike,
    ToggleDislike,
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
            LocalCommand::ChangeVolume(_) => "changeVolume",
        }
    }

    pub fn expr(self) -> String {
        match self {
            LocalCommand::PlayPause => format!("{JS_CONTROLLER_NAME}.playPause()"),
            LocalCommand::Next => format!("{JS_CONTROLLER_NAME}.next()"),
            LocalCommand::Prev => format!("{JS_CONTROLLER_NAME}.prev()"),
            LocalCommand::ToggleLike => format!("{JS_CONTROLLER_NAME}.toggleLike()"),
            LocalCommand::ToggleDislike => format!("{JS_CONTROLLER_NAME}.toggleDislike()"),
            LocalCommand::ChangeVolume(action) => {
                let (name, value) = match action {
                    VolumeAction::Up => ("UP", 0u8),
                    VolumeAction::Down => ("DOWN", 0),
                    VolumeAction::Set(p) => ("SET", p),
                    VolumeAction::Mute => ("MUTE", 0),
                };
                format!("{JS_CONTROLLER_NAME}.changeVolume('{name}', {value})")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expr_strings() {
        assert_eq!(LocalCommand::PlayPause.expr(), "window._PyYMController.playPause()");
        assert_eq!(LocalCommand::Next.expr(), "window._PyYMController.next()");
        assert_eq!(LocalCommand::Prev.expr(), "window._PyYMController.prev()");
        assert_eq!(LocalCommand::ToggleLike.expr(), "window._PyYMController.toggleLike()");
        assert_eq!(LocalCommand::ToggleDislike.expr(), "window._PyYMController.toggleDislike()");
        assert_eq!(
            LocalCommand::ChangeVolume(VolumeAction::Set(35)).expr(),
            "window._PyYMController.changeVolume('SET', 35)"
        );
        assert_eq!(
            LocalCommand::ChangeVolume(VolumeAction::Up).expr(),
            "window._PyYMController.changeVolume('UP', 0)"
        );
        assert_eq!(
            LocalCommand::ChangeVolume(VolumeAction::Down).expr(),
            "window._PyYMController.changeVolume('DOWN', 0)"
        );
        assert_eq!(
            LocalCommand::ChangeVolume(VolumeAction::Mute).expr(),
            "window._PyYMController.changeVolume('MUTE', 0)"
        );
    }

    #[test]
    fn kind_strings() {
        assert_eq!(LocalCommand::PlayPause.kind(), "playPause");
        assert_eq!(LocalCommand::Next.kind(), "next");
        assert_eq!(LocalCommand::Prev.kind(), "prev");
        assert_eq!(LocalCommand::ToggleLike.kind(), "toggleLike");
        assert_eq!(LocalCommand::ToggleDislike.kind(), "toggleDislike");
        assert_eq!(LocalCommand::ChangeVolume(VolumeAction::Mute).kind(), "changeVolume");
    }
}
