var websocket = null;
var uuid = null;
var actionInfo = null;
var pluginUUID = null;
var settings = {};
var globalSettings = {};

function openTokenPopup() {
    const url = 'token.html';
    const name = 'YandexTokenPopup';
    const specs = 'width=500,height=650,left=200,top=200';
    window.open(url, name, specs);
}

window.updateToken = function(newToken) {
    globalSettings.token = newToken;
    saveGlobalSettings();
    alert("Token updated successfully!");
};

function updateLocalPort() {
    const val = document.getElementById("local_port_input").value;
    globalSettings.local_port = val;
    saveGlobalSettings();
}

function updateCheckbox(id, settingKey) {
     const val = document.getElementById(id).checked;
     settings[settingKey] = val;
     updateSettings();
}

function applyModeToAll() {
     if (websocket && websocket.readyState === 1) {
        var json = {
            "event": "sendToPlugin",
            "context": uuid,
            "payload": {
                "event": "applySettingsToAll",
                "settings": {
                    "control_mode": settings.control_mode
                }
            }
        };
        websocket.send(JSON.stringify(json));
        alert("Mode applied to all buttons active in this session.");
    }
}

function updateModeUI(mode) {
    const desc = document.getElementById("mode_description");
    const localGroup = document.getElementById("local_settings_group");
    const tokenBlock = document.getElementById("token_settings_block");
    
    const cleanMode = (mode || "local").toString().trim().toLowerCase();
    
    if (cleanMode === 'local') {
        if(desc) desc.innerHTML = "Управление клиентом на этом компьютере";
        if(localGroup) localGroup.classList.remove("hidden");
        if(tokenBlock) tokenBlock.classList.add("hidden");
    } else {
        if(desc) desc.innerHTML = "Удаленное управление через протокол ynison (бета)";
        if(localGroup) localGroup.classList.add("hidden");
        if(tokenBlock) tokenBlock.classList.remove("hidden");
    }
}

function initCustomSelect(selectedId, itemsId, settingKey, callback) {
    const selected = document.getElementById(selectedId);
    const items = document.getElementById(itemsId);
    const options = items.querySelectorAll("div");

    selected.addEventListener("click", function(e) {
        e.stopPropagation();
        closeAllSelect(this);
        items.classList.toggle("select-hide");
        this.classList.toggle("select-arrow-active");
    });

    options.forEach(option => {
        option.addEventListener("click", function() {
            const val = this.getAttribute("data-value");
            selected.innerText = this.innerText;
            settings[settingKey] = val;
            
            options.forEach(opt => opt.classList.remove("same-as-selected"));
            this.classList.add("same-as-selected");
            
            updateSettings();
            items.classList.add("select-hide");
            selected.classList.remove("select-arrow-active");
            
            if (callback) callback(val);
        });
    });
}

function closeAllSelect(elmnt) {
    const allItems = document.querySelectorAll(".select-items");
    const allSelected = document.querySelectorAll(".select-selected");
    allSelected.forEach((sel, i) => {
        if (elmnt !== sel) {
            allItems[i].classList.add("select-hide");
            sel.classList.remove("select-arrow-active");
        }
    });
}

document.addEventListener("click", closeAllSelect);

function connectElgatoStreamDeckSocket(inPort, inPropertyInspectorUUID, inRegisterEvent, inInfo, inActionInfo) {
    uuid = inPropertyInspectorUUID;
    actionInfo = JSON.parse(inActionInfo);
    var info = JSON.parse(inInfo);
    pluginUUID = info.pluginUUID;
    
    websocket = new WebSocket('ws://localhost:' + inPort);

    websocket.onopen = function() {
        var json = { "event": inRegisterEvent, "uuid": inPropertyInspectorUUID };
        websocket.send(JSON.stringify(json));
        
        var jsonSettings = {
            "event": "getSettings",
            "context": uuid
        };
        websocket.send(JSON.stringify(jsonSettings));

        var jsonGlobal = {
            "event": "getGlobalSettings",
            "context": pluginUUID
        };
        websocket.send(JSON.stringify(jsonGlobal));
        
        initCustomSelect("control_mode_selected", "control_mode_items", "control_mode", updateModeUI);
        
        const action = actionInfo.action;
        if (action === "com.judd1.yandex_music.action.info") {
            document.getElementById('info_settings').classList.remove('hidden');
        } else if (action === "com.judd1.yandex_music.action.like") {
            document.getElementById('like_settings').classList.remove('hidden');
            initCustomSelect("like_style_selected", "like_style_items", "like_style");
        } else if (action === "com.judd1.yandex_music.action.dislike") {
            document.getElementById('dislike_settings').classList.remove('hidden');
            initCustomSelect("dislike_style_selected", "dislike_style_items", "dislike_style");
        } else if (action === "com.judd1.yandex_music.action.next") {
            document.getElementById('next_settings').classList.remove('hidden');
            initCustomSelect("next_style_selected", "next_style_items", "next_style");
        } else if (action === "com.judd1.yandex_music.action.prev") {
            document.getElementById('prev_settings').classList.remove('hidden');
            initCustomSelect("prev_style_selected", "prev_style_items", "prev_style");
        } else if (action === "com.judd1.yandex_music.action.playpause") {
            document.getElementById('play_settings').classList.remove('hidden');
            initCustomSelect("play_style_selected", "play_style_items", "play_style");
        } else if (action === "com.judd1.yandex_music.action.progress") {
            document.getElementById('progress_settings').classList.remove('hidden');
            initCustomSelect("progress_mode_selected", "progress_mode_items", "progress_mode");
        } else if (action === "com.judd1.yandex_music.action.mute") {
            document.getElementById('mute_settings').classList.remove('hidden');
            initCustomSelect("mute_style_selected", "mute_style_items", "mute_style");
        } else if (action === "com.judd1.yandex_music.action.volumeup" || 
                   action === "com.judd1.yandex_music.action.volumedown" || 
                   action === "com.judd1.yandex_music.action.volume_display") {
            document.getElementById('volume_settings').classList.remove('hidden');
            initCustomSelect("volume_style_selected", "volume_style_items", "volume_style");
        }

        if (actionInfo.payload.settings) {
            settings = actionInfo.payload.settings;
        }
        
        if (!settings.control_mode) settings.control_mode = "local";
        
        syncUIRomSettings();
        updateModeUI(settings.control_mode);
    };

    websocket.onmessage = function(evt) {
        var jsonObj = JSON.parse(evt.data);
        if (jsonObj.event === 'didReceiveSettings') {
            settings = jsonObj.payload.settings;
            syncUIRomSettings();
            updateModeUI(settings.control_mode);
        } else if (jsonObj.event === 'didReceiveGlobalSettings') {
            globalSettings = jsonObj.payload.settings;
            if (globalSettings.local_port) {
                document.getElementById("local_port_input").value = globalSettings.local_port;
            }
        } else if (jsonObj.event === 'sendToPropertyInspector') {
            var payload = jsonObj.payload;
            if (payload.event === "TokenStatus") {
                updateStatusIndicator("token_status_indicator", payload.status);
            } else if (payload.event === "LocalStatus") {
                 updateStatusIndicator("local_status_indicator", payload.status);
            }
        }
    };
}

function updateStatusIndicator(id, status) {
    const el = document.getElementById(id);
    if (!el) return;
    
    if (status === "valid" || status === "connected") {
        el.textContent = "ПОДКЛЮЧЕНО";
        el.style.background = "#1b5e20"; 
        el.style.color = "#a5d6a7";
    } else if (status === "invalid") {
        el.textContent = "НЕВАЛИДНО";
        el.style.background = "#b71c1c"; 
        el.style.color = "#ffcdd2";
    } else if (status === "offline" || status === "disconnected") {
        el.textContent = "ОФФЛАЙН";
        el.style.background = "#424242"; 
        el.style.color = "#bdbdbd";
    } else if (status === "loading") {
         el.textContent = "ЗАГРУЗКА...";
        el.style.background = "#ff6f00"; 
        el.style.color = "#ffe0b2";
    } else {
        el.textContent = "UNKNOWN";
        el.style.background = "#ff6f00"; 
        el.style.color = "#ffe0b2";
    }
}

function syncUIRomSettings() {
    const syncSelect = (key, itemsId, selectedId) => {
         if (settings[key]) {
            const items = document.getElementById(itemsId);
            if (items) {
                const options = items.querySelectorAll("div");
                options.forEach(opt => {
                    if (opt.getAttribute("data-value") === settings[key]) {
                        const selEl = document.getElementById(selectedId);
                        if(selEl) {
                            selEl.innerText = opt.innerText;
                            options.forEach(o => o.classList.remove("same-as-selected"));
                            opt.classList.add("same-as-selected");
                        }
                    }
                });
            }
        }
    }
    const syncCheck = (key, id) => {
        const val = settings[key] !== false; 
        const el = document.getElementById(id);
        if (el) el.checked = val;
    }
    
    syncSelect("like_style", "like_style_items", "like_style_selected");
    syncSelect("dislike_style", "dislike_style_items", "dislike_style_selected");
    syncSelect("next_style", "next_style_items", "next_style_selected");
    syncSelect("prev_style", "prev_style_items", "prev_style_selected");
    syncSelect("play_style", "play_style_items", "play_style_selected");
    syncSelect("progress_mode", "progress_mode_items", "progress_mode_selected");
    syncSelect("control_mode", "control_mode_items", "control_mode_selected");
    syncSelect("mute_style", "mute_style_items", "mute_style_selected");
    syncSelect("volume_style", "volume_style_items", "volume_style_selected");
    
    syncCheck("show_cover", "chk_show_cover");
    syncCheck("show_title", "chk_show_title");
    syncCheck("show_artist", "chk_show_artist");
}

function updateSettings() {
    if (websocket && websocket.readyState === 1) {
        var json = {
            "event": "setSettings",
            "context": uuid,
            "payload": settings
        };
        websocket.send(JSON.stringify(json));
    }
}

function saveGlobalSettings() {
    if (websocket && websocket.readyState === 1) {
        var json = {
            "event": "setGlobalSettings",
            "context": uuid,
            "payload": globalSettings
        };
        websocket.send(JSON.stringify(json));
    }
}
