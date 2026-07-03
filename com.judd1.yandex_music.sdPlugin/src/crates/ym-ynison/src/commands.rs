#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum YnisonCommand {
    PlayPause,
    Next,
    Prev,
    Like,
    Dislike,
    VolumeUp,
    VolumeDown,
}

impl YnisonCommand {
    pub fn as_str(self) -> &'static str {
        match self {
            YnisonCommand::PlayPause => "play_pause",
            YnisonCommand::Next => "next",
            YnisonCommand::Prev => "prev",
            YnisonCommand::Like => "like",
            YnisonCommand::Dislike => "dislike",
            YnisonCommand::VolumeUp => "volume_up",
            YnisonCommand::VolumeDown => "volume_down",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_strings() {
        assert_eq!(YnisonCommand::PlayPause.as_str(), "play_pause");
        assert_eq!(YnisonCommand::VolumeUp.as_str(), "volume_up");
        assert_eq!(YnisonCommand::Prev.as_str(), "prev");
    }
}
