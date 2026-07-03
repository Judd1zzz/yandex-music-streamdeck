var websocket = null;
var uuid = null;
var actionInfo = null;
var pluginUUID = null;
var settings = {};
var globalSettings = {};
var globalsReady = false;
var toastTimer = null;

function showToast(text) {
    const el = document.getElementById("pi_toast");
    if (!el) return;
    el.textContent = text;
    el.classList.remove("hidden");
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(function() { el.classList.add("hidden"); }, 2500);
}

function openTokenPopup() {
    const url = 'token.html';
    const name = 'YandexTokenPopup';
    const specs = 'width=500,height=650,left=200,top=200';
    window.open(url, name, specs);
}

window.updateToken = function(newToken) {
    globalSettings.token = newToken;
    saveGlobalSettings();
    showToast("Токен сохранён");
};

function updateLocalPort() {
    const input = document.getElementById("local_port_input");
    let port = parseInt(input.value, 10);
    if (!port || port < 1 || port > 65535) port = 9222;
    input.value = port;
    globalSettings.local_port = port;
    saveGlobalSettings();
}

function updateKnobStep() {
    const input = document.getElementById("knob_step_input");
    let step = parseInt(input.value, 10);
    if (!step || step < 1) step = 5;
    if (step > 20) step = 20;
    input.value = step;
    settings.knob_step = step;
    updateSettings();
}

function updateClientAutofix() {
    globalSettings.client_autofix_enabled = document.getElementById("chk_client_autofix").checked;
    saveGlobalSettings();
}

function updateClientPath() {
    globalSettings.client_exe_path = document.getElementById("client_path_input").value;
    saveGlobalSettings();
}

function updateDiscordEnabled() {
    globalSettings.discord_rpc_enabled = document.getElementById("chk_discord_rpc").checked;
    saveGlobalSettings();
}

function updateDiscordAppId() {
    globalSettings.discord_app_id = document.getElementById("discord_app_id_input").value;
    saveGlobalSettings();
}

function updateCheckbox(id, settingKey) {
     const val = document.getElementById(id).checked;
     settings[settingKey] = val;
     updateSettings();
}

function updateDownloadPath() {
    globalSettings.download_path = document.getElementById("download_path_input").value;
    saveGlobalSettings();
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
        showToast("Режим применён ко всем кнопкам");
    } else {
        showToast("Нет связи с плагином");
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

function initSelect(selectedId, itemsId, write, callback, guard) {
    const selected = document.getElementById(selectedId);
    const items = document.getElementById(itemsId);
    if (!selected || !items) return;
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
            const clicked = this;
            items.classList.add("select-hide");
            selected.classList.remove("select-arrow-active");

            const apply = function() {
                selected.textContent = clicked.textContent;
                options.forEach(opt => opt.classList.remove("same-as-selected"));
                clicked.classList.add("same-as-selected");
                write(val);
                if (callback) callback(val);
            };

            if (guard && !guard(val, apply)) return;
            apply();
        });
    });
}

function initCustomSelect(selectedId, itemsId, settingKey, callback, guard) {
    initSelect(selectedId, itemsId, function(val) {
        settings[settingKey] = val;
        updateSettings();
    }, callback, guard);
}

function ynisonGuard(val, apply) {
    if (val !== "ynison" || settings.control_mode === "ynison") return true;
    showYnisonModal(apply);
    return false;
}

function showYnisonModal(onConfirm) {
    const modal = document.getElementById("ynison_modal");
    if (!modal) return;
    modal.classList.remove("hidden");
    const close = function() {
        modal.classList.add("hidden");
        document.removeEventListener("keydown", onKey);
    };
    const onKey = function(e) {
        if (e.key === "Escape") close();
    };
    document.addEventListener("keydown", onKey);
    const cancel = document.getElementById("ynison_cancel");
    cancel.onclick = close;
    cancel.focus();
    document.getElementById("ynison_confirm").onclick = function() {
        close();
        onConfirm();
    };
}

function initGlobalSelect(selectedId, itemsId, globalKey) {
    initSelect(selectedId, itemsId, function(val) {
        globalSettings[globalKey] = val;
        saveGlobalSettings();
    });
}

function syncGlobalSelect(globalKey, itemsId, selectedId) {
    const val = globalSettings[globalKey];
    if (!val) return;
    const items = document.getElementById(itemsId);
    if (!items) return;
    const options = items.querySelectorAll("div");
    options.forEach(opt => {
        if (opt.getAttribute("data-value") === val) {
            const sel = document.getElementById(selectedId);
            if (sel) sel.innerText = opt.innerText;
            options.forEach(o => o.classList.remove("same-as-selected"));
            opt.classList.add("same-as-selected");
        }
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
            "context": uuid
        };
        websocket.send(JSON.stringify(jsonGlobal));
        setTimeout(function() {
            if (!globalsReady && websocket && websocket.readyState === 1) {
                websocket.send(JSON.stringify(jsonGlobal));
            }
        }, 2000);
        
        initCustomSelect("control_mode_selected", "control_mode_items", "control_mode", updateModeUI, ynisonGuard);
        initGlobalSelect("download_format_selected", "download_format_items", "download_format");

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
        } else if (action === "com.judd1.yandex_music.action.volume_knob") {
            document.getElementById('knob_settings').classList.remove('hidden');
            document.getElementById('volume_settings').classList.remove('hidden');
            initCustomSelect("knob_press_selected", "knob_press_items", "knob_press");
            initCustomSelect("volume_style_selected", "volume_style_items", "volume_style");
        } else if (action === "com.judd1.yandex_music.action.download") {
            document.getElementById('download_settings').classList.remove('hidden');
            initCustomSelect("download_style_selected", "download_style_items", "download_style");
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
            globalSettings = jsonObj.payload.settings || {};
            globalsReady = true;
            if (globalSettings.local_port) {
                document.getElementById("local_port_input").value = globalSettings.local_port;
            }
            const autofixChk = document.getElementById("chk_client_autofix");
            if (autofixChk) autofixChk.checked = globalSettings.client_autofix_enabled !== false;
            const clientPath = document.getElementById("client_path_input");
            if (clientPath && typeof globalSettings.client_exe_path === "string") clientPath.value = globalSettings.client_exe_path;
            const discordChk = document.getElementById("chk_discord_rpc");
            if (discordChk) discordChk.checked = globalSettings.discord_rpc_enabled === true;
            const discordApp = document.getElementById("discord_app_id_input");
            if (discordApp && globalSettings.discord_app_id) discordApp.value = globalSettings.discord_app_id;
            const dlPath = document.getElementById("download_path_input");
            if (dlPath && typeof globalSettings.download_path === "string") dlPath.value = globalSettings.download_path;
            syncGlobalSelect("download_format", "download_format_items", "download_format_selected");
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
    syncSelect("download_style", "download_style_items", "download_style_selected");
    syncSelect("knob_press", "knob_press_items", "knob_press_selected");

    const knobStep = document.getElementById("knob_step_input");
    if (knobStep && settings.knob_step !== undefined) knobStep.value = settings.knob_step;

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
    if (!globalsReady) return;
    if (websocket && websocket.readyState === 1) {
        var json = {
            "event": "setGlobalSettings",
            "context": uuid,
            "payload": globalSettings
        };
        websocket.send(JSON.stringify(json));
    }
}
