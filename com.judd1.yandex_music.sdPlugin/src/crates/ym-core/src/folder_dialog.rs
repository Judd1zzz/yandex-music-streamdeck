use crate::action::PickKind;

pub fn pick(kind: PickKind) -> Option<String> {
    let out = run(kind)?;
    let trimmed = out.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

#[cfg(target_os = "macos")]
fn run(kind: PickKind) -> Option<String> {
    let script = match kind {
        PickKind::Folder => {
            "POSIX path of (choose folder with prompt \"Select the download folder\")".to_owned()
        }
        PickKind::ClientApp => {
            "POSIX path of (choose file with prompt \"Select the Yandex Music app\" of type {\"com.apple.application-bundle\"})".to_owned()
        }
    };
    let output = std::process::Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "windows")]
fn run(kind: PickKind) -> Option<String> {
    use std::os::windows::process::CommandExt;
    let script = match kind {
        PickKind::Folder => {
            "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; $d.Description = 'Select the download folder'; if ($d.ShowDialog() -eq 'OK') { [Console]::Out.Write([Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes($d.SelectedPath))) }"
        }
        PickKind::ClientApp => {
            "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Title = 'Select the Yandex Music client'; $d.Filter = 'Executable (*.exe)|*.exe'; if ($d.ShowDialog() -eq 'OK') { [Console]::Out.Write([Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes($d.FileName))) }"
        }
    };
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-STA", "-Command", script])
        .creation_flags(0x0800_0000)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    decode_b64_stdout(&output.stdout)
}

#[cfg(any(target_os = "windows", test))]
fn decode_b64_stdout(stdout: &[u8]) -> Option<String> {
    use base64::Engine;
    let raw = String::from_utf8_lossy(stdout);
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(trimmed)
        .map_err(|e| tracing::warn!("пикер: stdout диалога не base64: {e}"))
        .ok()?;
    String::from_utf8(bytes)
        .map_err(|e| tracing::warn!("пикер: путь из диалога не UTF-8: {e}"))
        .ok()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn run(kind: PickKind) -> Option<String> {
    let mut cmd = std::process::Command::new("zenity");
    cmd.arg("--file-selection");
    match kind {
        PickKind::Folder => {
            cmd.arg("--directory");
        }
        PickKind::ClientApp => {}
    }
    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::decode_b64_stdout;
    use base64::Engine;

    fn b64(bytes: &[u8]) -> String {
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    #[test]
    fn decodes_cyrillic_path_with_trailing_newline() {
        let path = "C:\\Users\\Тест\\AppData\\Local\\Programs\\YandexMusic\\Яндекс Музыка.exe";
        let stdout = format!("{}\r\n", b64(path.as_bytes()));
        assert_eq!(decode_b64_stdout(stdout.as_bytes()).as_deref(), Some(path));
    }

    #[test]
    fn rejects_plain_text_stdout() {
        assert_eq!(decode_b64_stdout(b"C:\\Users\\??????\\app.exe"), None);
    }

    #[test]
    fn rejects_invalid_utf8_payload() {
        assert_eq!(decode_b64_stdout(b64(&[0xFF, 0xFE, 0x41]).as_bytes()), None);
    }

    #[test]
    fn empty_stdout_means_cancel() {
        assert_eq!(decode_b64_stdout(b""), None);
        assert_eq!(decode_b64_stdout(b"\r\n"), None);
    }
}
