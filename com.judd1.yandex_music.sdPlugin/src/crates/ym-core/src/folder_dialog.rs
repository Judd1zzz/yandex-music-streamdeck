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
    let output = std::process::Command::new("osascript").arg("-e").arg(&script).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

#[cfg(target_os = "windows")]
fn run(kind: PickKind) -> Option<String> {
    let script = match kind {
        PickKind::Folder => {
            "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.FolderBrowserDialog; $d.Description = 'Select the download folder'; if ($d.ShowDialog() -eq 'OK') { [Console]::Out.Write($d.SelectedPath) }"
        }
        PickKind::ClientApp => {
            "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Title = 'Select the Yandex Music client'; $d.Filter = 'Executable (*.exe)|*.exe'; if ($d.ShowDialog() -eq 'OK') { [Console]::Out.Write($d.FileName) }"
        }
    };
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-STA", "-Command", script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
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
