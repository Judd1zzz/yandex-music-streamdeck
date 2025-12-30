import os
import sys
import json
import shutil
import subprocess


DIST_DIR = "dist"
PLUGIN_UUID = "com.judd1.yandex_music.sdPlugin"
PLUGIN_DIR = os.path.join(DIST_DIR, PLUGIN_UUID)
SRC_INTERNAL = os.path.join(PLUGIN_DIR, "_internal")


def clean():
    if os.path.exists(DIST_DIR):
        print(f"Cleaning {DIST_DIR}...")
        shutil.rmtree(DIST_DIR)

def build_binary():
    print("Running PyInstaller...")
    subprocess.check_call([
        "../env/bin/pyinstaller" if sys.platform == "darwin" else r"../env/Scripts/pyinstaller",  # ВАЖНО: подправить путь, если сборка с ошибкой падает
        "main.spec", 
        "--noconfirm", 
        "--log-level=WARN"
    ])

def copy_assets():
    print("Copying assets...")
    
    shutil.copy("manifest.json", os.path.join(PLUGIN_DIR, "manifest.json"))
    
    dest_static = os.path.join(PLUGIN_DIR, "static")
    if os.path.exists(dest_static):
        shutil.rmtree(dest_static)
    shutil.copytree("static", dest_static)
    
    print(f"Assets copied to {PLUGIN_DIR}")

def update_manifest():
    print("Updating manifest.json...")
    manifest_path = os.path.join(PLUGIN_DIR, "manifest.json")
    
    with open(manifest_path, "r", encoding="utf-8") as f:
        data = json.load(f)
        
    data["CodePathMac"] = PLUGIN_UUID
    data["CodePathWin"] = f"{PLUGIN_UUID}.exe"
    
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=4, ensure_ascii=False)
        
    print("Manifest updated.")

def main():
    try:
        clean()
        build_binary()
        copy_assets()
        update_manifest()
        
        print("\n" + "="*30)
        print(f"SUCCESS! Plugin built at: {PLUGIN_DIR}")
        print("="*30)
        
    except Exception as e:
        print(f"\nERROR: Build failed: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
