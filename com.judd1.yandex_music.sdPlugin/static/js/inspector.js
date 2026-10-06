var websocket = null;
var uuid = null;
var actionInfo = null;
var pluginUUID = null;
var settings = {};
var globalSettings = {};
var globalsReady = false;
var piLang = "en";
var lastLocalStatus = null;
var lastLocalReason = "";
var lastUpdateVersion = "";
var lastDiscordStatus = null;

var I18N = {
    en: {
        client_status_label: "Yandex Music client",
        client_label: "Client",
        autofix_label: "Launch/relaunch the client automatically",
        client_path_label: "Client path",
        ph_client_path: "Empty — auto-detect",
        autofix_hint: "The plugin finds the Yandex Music client on its own. Set the path only if the client is installed in a non-standard location.",
        info_elements_label: "Displayed elements",
        show_cover_label: "Cover art",
        show_title_label: "Track title",
        show_artist_label: "Artist name",
        show_timecodes_label: "Timecodes (elapsed / total)",
        info_press_label: "Press action",
        opt_info_press_copy: "Copy track name",
        opt_info_press_none: "Do nothing",
        btn_style_label: "Button style",
        opt_style_v1: "original (with background)",
        progress_format_label: "Display format",
        knob_press_label: "Knob press",
        opt_knob_mute: "Mute (toggle sound)",
        knob_step_label: "Step per tick, %",
        discord_rpc_label: "Rich Presence (current track in your profile)",
        ph_discord_app: "Empty — built-in application",
        discord_hint: "Leave empty to use the built-in application. A custom ID is only needed if you want a custom name/icon.",
        download_dir_label: "Download folder",
        ph_download_path: "default: Music",
        format_label: "Format",
        lang_label: "Language",
        settings_note: "Settings are saved immediately",
        btn_open_dashboard: "Open settings dashboard",
        path_ok: "✓ Client found",
        path_ok_dir: "Folder given — {file} will be used",
        path_missing: "Path does not exist",
        path_no_client: "No “{file}” in this folder — point to {file}",
        status_connected: "CONNECTED",
        status_invalid: "INVALID",
        status_offline: "OFFLINE",
        status_loading: "LOADING...",
        update_notice: "🔄 Update {v} installed — restart Stream Deck. ",
        whats_new: "What's new",
        reason_bridge_missing: "The bridge library isn't next to the plugin — antivirus may have quarantined it. Restore the file or reinstall the plugin from the release archive",
        reason_bridge_untrusted: "The bridge library's signature didn't verify — reinstall the plugin from the release archive",
        reason_bridge_arch_mismatch: "The bridge library has no slice for this Mac's processor — the plugin runs without control until a matching build is installed",
        reason_client_elevated: "The client is running as administrator — the plugin cannot control it. Untick “Run this program as an administrator” in the client shortcut properties",
        reason_elevation_declined: "The administrator rights request was declined — press any plugin button to try again",
        reason_client_not_found: "Yandex Music client not found — install it from music.yandex.ru/download or set the path in the settings"
    },
    ru: {
        client_status_label: "Клиент Яндекс Музыки",
        client_label: "Клиент",
        autofix_label: "Запускать/перезапускать клиент автоматически",
        client_path_label: "Путь к клиенту",
        ph_client_path: "Пусто — автоопределение",
        autofix_hint: "Плагин сам находит клиент Яндекс Музыки. Путь указывайте, только если клиент установлен в нестандартное место.",
        info_elements_label: "Отображаемые элементы",
        show_cover_label: "Обложка",
        show_title_label: "Название трека",
        show_artist_label: "Имя исполнителя",
        show_timecodes_label: "Тайм-коды (прошло / всего)",
        info_press_label: "Действие по нажатию",
        opt_info_press_copy: "Копировать название трека",
        opt_info_press_none: "Ничего не делать",
        btn_style_label: "Стиль кнопки",
        opt_style_v1: "original (с фоном)",
        progress_format_label: "Формат отображения",
        knob_press_label: "Нажатие крутилки",
        opt_knob_mute: "Mute (выкл/вкл звук)",
        knob_step_label: "Шаг за тик, %",
        discord_rpc_label: "Rich Presence (текущий трек в профиле)",
        ph_discord_app: "Пусто — встроенное приложение",
        discord_hint: "Оставьте пустым — используется встроенное приложение. Свой ID нужен, только если хотите отдельное имя/иконку.",
        download_dir_label: "Папка скачивания",
        ph_download_path: "по умолчанию: Музыка",
        format_label: "Формат",
        lang_label: "Язык",
        settings_note: "Настройки сохраняются сразу",
        btn_open_dashboard: "Открыть панель настроек",
        path_ok: "✓ Клиент найден",
        path_ok_dir: "Указана папка — будет использован {file}",
        path_missing: "Путь не существует",
        path_no_client: "В папке нет «{file}» — укажите путь к {file}",
        status_connected: "ПОДКЛЮЧЕНО",
        status_invalid: "НЕВАЛИДНО",
        status_offline: "ОФФЛАЙН",
        status_loading: "ЗАГРУЗКА...",
        update_notice: "🔄 Установлено обновление {v} — перезапустите Stream Deck. ",
        whats_new: "Что нового",
        reason_bridge_missing: "Рядом с плагином нет библиотеки-моста — возможно, её убрал в карантин антивирус. Восстановите файл или переустановите плагин из архива релиза",
        reason_bridge_untrusted: "Подпись библиотеки-моста не сошлась — переустановите плагин из архива релиза",
        reason_bridge_arch_mismatch: "В библиотеке-мосте нет среза под процессор этого Mac — до установки подходящей сборки плагин работает без управления",
        reason_client_elevated: "Клиент запущен от имени администратора — плагин не может им управлять. Снимите галочку «Запускать эту программу от имени администратора» в свойствах ярлыка клиента",
        reason_elevation_declined: "Запрос прав администратора отклонён — нажмите любую кнопку плагина, чтобы попробовать снова",
        reason_client_not_found: "Клиент Яндекс Музыки не найден — установите его с music.yandex.ru/download или укажите путь в настройках"
    }
};
window.I18N = I18N;

function t(key, vars) {
    var dict = I18N[piLang] || I18N.en;
    var s = dict[key];
    if (s == null) s = I18N.en[key];
    if (s == null) return key;
    if (vars) {
        Object.keys(vars).forEach(function(k) {
            s = s.split("{" + k + "}").join(String(vars[k]));
        });
    }
    return s;
}

function resyncSelectLabels() {
    document.querySelectorAll(".custom-select").forEach(function(box) {
        var selected = box.querySelector(".select-selected");
        var items = box.querySelector(".select-items");
        if (!selected || !items) return;
        var current = items.querySelector(".same-as-selected");
        if (!current && selected.getAttribute("data-default-value")) {
            current = items.querySelector('[data-value="' + selected.getAttribute("data-default-value") + '"]');
        }
        if (current) selected.textContent = current.textContent;
    });
}

function rerenderDynamicTexts() {
    if (lastLocalStatus) updateStatusIndicator("local_status_indicator", lastLocalStatus);
    updateLocalReason(lastLocalReason);
    if (lastUpdateVersion) renderUpdateNotice(lastUpdateVersion);
}

function applyLanguage(lang) {
    piLang = lang === "ru" ? "ru" : "en";
    document.documentElement.lang = piLang;
    document.querySelectorAll("[data-i18n]").forEach(function(el) {
        el.textContent = t(el.getAttribute("data-i18n"));
    });
    document.querySelectorAll("[data-i18n-placeholder]").forEach(function(el) {
        el.setAttribute("placeholder", t(el.getAttribute("data-i18n-placeholder")));
    });
    resyncSelectLabels();
    rerenderDynamicTexts();
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

function updateCheckbox(id, settingKey) {
     const val = document.getElementById(id).checked;
     settings[settingKey] = val;
     updateSettings();
}

function initSelect(selectedId, itemsId, write, callback) {
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

            selected.textContent = clicked.textContent;
            options.forEach(opt => opt.classList.remove("same-as-selected"));
            clicked.classList.add("same-as-selected");
            write(val);
            if (callback) callback(val);
        });
    });
}

function initCustomSelect(selectedId, itemsId, settingKey, callback) {
    initSelect(selectedId, itemsId, function(val) {
        settings[settingKey] = val;
        updateSettings();
    }, callback);
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
        
        const rawAction = String(actionInfo.action || '');
        const action = rawAction.replace(/_/g, '-');
        if (action === "com.judd1.yandex-music.action.info") {
            document.getElementById('info_settings').classList.remove('hidden');
            initCustomSelect("info_press_selected", "info_press_items", "info_press");
        } else if (action === "com.judd1.yandex-music.action.launch") {
            document.getElementById('client_status').classList.add('hidden');
        } else if (action === "com.judd1.yandex-music.action.like") {
            document.getElementById('like_settings').classList.remove('hidden');
            initCustomSelect("like_style_selected", "like_style_items", "like_style");
        } else if (action === "com.judd1.yandex-music.action.dislike") {
            document.getElementById('dislike_settings').classList.remove('hidden');
            initCustomSelect("dislike_style_selected", "dislike_style_items", "dislike_style");
        } else if (action === "com.judd1.yandex-music.action.next") {
            document.getElementById('next_settings').classList.remove('hidden');
            initCustomSelect("next_style_selected", "next_style_items", "next_style");
        } else if (action === "com.judd1.yandex-music.action.prev") {
            document.getElementById('prev_settings').classList.remove('hidden');
            initCustomSelect("prev_style_selected", "prev_style_items", "prev_style");
        } else if (action === "com.judd1.yandex-music.action.playpause") {
            document.getElementById('play_settings').classList.remove('hidden');
            initCustomSelect("play_style_selected", "play_style_items", "play_style");
        } else if (action === "com.judd1.yandex-music.action.progress") {
            document.getElementById('progress_settings').classList.remove('hidden');
            initCustomSelect("progress_mode_selected", "progress_mode_items", "progress_mode");
        } else if (action === "com.judd1.yandex-music.action.mute") {
            document.getElementById('mute_settings').classList.remove('hidden');
            initCustomSelect("mute_style_selected", "mute_style_items", "mute_style");
        } else if (action === "com.judd1.yandex-music.action.volumeup" ||
                   action === "com.judd1.yandex-music.action.volumedown" ||
                   action === "com.judd1.yandex-music.action.volume-display") {
            document.getElementById('volume_settings').classList.remove('hidden');
            initCustomSelect("volume_style_selected", "volume_style_items", "volume_style");
        } else if (action === "com.judd1.yandex-music.action.volume-knob") {
            document.getElementById('knob_settings').classList.remove('hidden');
            document.getElementById('volume_settings').classList.remove('hidden');
            initCustomSelect("knob_press_selected", "knob_press_items", "knob_press");
            initCustomSelect("volume_style_selected", "volume_style_items", "volume_style");
        } else if (action === "com.judd1.yandex-music.action.download") {
            document.getElementById('download_settings').classList.remove('hidden');
            initCustomSelect("download_style_selected", "download_style_items", "download_style");
        } else if (action === "com.judd1.yandex-music.action.shuffle") {
            document.getElementById('shuffle_settings').classList.remove('hidden');
            initCustomSelect("shuffle_style_selected", "shuffle_style_items", "shuffle_style");
        } else if (action === "com.judd1.yandex-music.action.repeat") {
            document.getElementById('repeat_settings').classList.remove('hidden');
            initCustomSelect("repeat_style_selected", "repeat_style_items", "repeat_style");
        }

        if (actionInfo.payload.settings) {
            settings = actionInfo.payload.settings;
        }

        syncUIRomSettings();
    };

    websocket.onmessage = function(evt) {
        var jsonObj = JSON.parse(evt.data);
        if (jsonObj.event === 'didReceiveSettings') {
            settings = jsonObj.payload.settings;
            syncUIRomSettings();
        } else if (jsonObj.event === 'didReceiveGlobalSettings') {
            globalSettings = jsonObj.payload.settings || {};
            delete globalSettings.token;
            globalsReady = true;
            applyLanguage(globalSettings.pi_language === "ru" ? "ru" : "en");
            postToDashboard("globalSettings", globalSettings);
        } else if (jsonObj.event === 'sendToPropertyInspector') {
            var payload = jsonObj.payload;
            if (payload.event === "LocalStatus") {
                lastLocalStatus = payload.status;
                updateStatusIndicator("local_status_indicator", payload.status);
                updateLocalReason(payload.status === "connected" ? "" : payload.reason);
            } else if (payload.event === "UpdateNotice") {
                renderUpdateNotice(payload.version);
            } else if (payload.event === "DiscordStatus") {
                lastDiscordStatus = payload.status;
            }
            postToDashboard("event", payload);
        }
    };
}

function openExternal(url) {
    if (!websocket) return;
    websocket.send(JSON.stringify({ event: "openUrl", payload: { url: url } }));
}

function renderUpdateNotice(version) {
    const el = document.getElementById("update_notice");
    if (!el) return;
    const v = (version || "").toString().trim();
    lastUpdateVersion = v;
    if (!v) {
        el.classList.add("hidden");
        return;
    }
    el.textContent = t("update_notice", { v: v });
    const link = document.createElement("span");
    link.className = "pi-update-link";
    link.textContent = t("whats_new");
    link.addEventListener("click", function () {
        openExternal("https://github.com/Judd1zzz/yandex-music-streamdeck/releases/tag/v" + v);
    });
    el.appendChild(link);
    el.classList.remove("hidden");
}

function localizeReason(reason) {
    const raw = (reason || "").toString();
    if (!raw) return "";
    if (I18N.en["reason_" + raw] != null) {
        return t("reason_" + raw);
    }
    return raw;
}

function updateLocalReason(reason) {
    const el = document.getElementById("local_status_reason");
    if (!el) return;
    lastLocalReason = reason || "";
    const text = localizeReason(lastLocalReason);
    el.textContent = text;
    el.classList.toggle("hidden", !text);
}

function updateStatusIndicator(id, status) {
    const el = document.getElementById(id);
    if (!el) return;

    if (status === "valid" || status === "connected") {
        el.textContent = t("status_connected");
        el.style.background = "#1b5e20";
        el.style.color = "#a5d6a7";
    } else if (status === "invalid") {
        el.textContent = t("status_invalid");
        el.style.background = "#b71c1c";
        el.style.color = "#ffcdd2";
    } else if (status === "offline" || status === "disconnected") {
        el.textContent = t("status_offline");
        el.style.background = "#424242";
        el.style.color = "#bdbdbd";
    } else if (status === "loading") {
         el.textContent = t("status_loading");
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
    const syncCheckOff = (key, id) => {
        const el = document.getElementById(id);
        if (el) el.checked = settings[key] === true;
    }

    syncSelect("like_style", "like_style_items", "like_style_selected");
    syncSelect("dislike_style", "dislike_style_items", "dislike_style_selected");
    syncSelect("next_style", "next_style_items", "next_style_selected");
    syncSelect("prev_style", "prev_style_items", "prev_style_selected");
    syncSelect("play_style", "play_style_items", "play_style_selected");
    syncSelect("progress_mode", "progress_mode_items", "progress_mode_selected");
    syncSelect("mute_style", "mute_style_items", "mute_style_selected");
    syncSelect("volume_style", "volume_style_items", "volume_style_selected");
    syncSelect("download_style", "download_style_items", "download_style_selected");
    syncSelect("shuffle_style", "shuffle_style_items", "shuffle_style_selected");
    syncSelect("repeat_style", "repeat_style_items", "repeat_style_selected");
    syncSelect("knob_press", "knob_press_items", "knob_press_selected");
    syncSelect("info_press", "info_press_items", "info_press_selected");

    const knobStep = document.getElementById("knob_step_input");
    if (knobStep && settings.knob_step !== undefined) knobStep.value = settings.knob_step;

    syncCheck("show_cover", "chk_show_cover");
    syncCheck("show_title", "chk_show_title");
    syncCheck("show_artist", "chk_show_artist");
    syncCheckOff("show_timecodes", "chk_show_timecodes");
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
    delete globalSettings.token;
    if (websocket && websocket.readyState === 1) {
        var json = {
            "event": "setGlobalSettings",
            "context": uuid,
            "payload": globalSettings
        };
        websocket.send(JSON.stringify(json));
    }
}

var dashboardWindow = null;

function collectSnapshot() {
    return {
        lang: piLang,
        action: actionInfo ? actionInfo.action : null,
        globalSettings: globalSettings,
        settings: settings,
        localStatus: lastLocalStatus,
        localReason: lastLocalReason,
        updateVersion: lastUpdateVersion,
        pathCheck: null,
        discord: lastDiscordStatus
    };
}

function postToDashboard(type, payload) {
    if (!dashboardWindow || dashboardWindow.closed) return;
    try {
        dashboardWindow.postMessage({ channel: "ym", type: type, payload: payload }, "*");
    } catch (e) {}
}

window.ymBridge = {
    ready: function() {
        postToDashboard("snapshot", collectSnapshot());
    },
    getSnapshot: function() {
        return collectSnapshot();
    },
    setGlobal: function(patch) {
        if (!patch || typeof patch !== "object") return;
        Object.keys(patch).forEach(function(k) { globalSettings[k] = patch[k]; });
        saveGlobalSettings();
        if (patch.pi_language && patch.pi_language !== piLang) {
            applyLanguage(patch.pi_language);
        }
    },
    sendToPlugin: function(payload) {
        if (websocket && websocket.readyState === 1) {
            var p = Object.assign({}, payload || {});
            if (actionInfo) {
                if (p.reply_action === undefined) p.reply_action = actionInfo.action;
                if (p.reply_context === undefined) p.reply_context = actionInfo.context;
            }
            websocket.send(JSON.stringify({ event: "sendToPlugin", context: uuid, payload: p }));
        }
    },
    openExternal: function(url) {
        openExternal(url);
    },
    checkClientPath: function(path) {
        if (websocket && websocket.readyState === 1) {
            websocket.send(JSON.stringify({
                event: "sendToPlugin",
                context: uuid,
                payload: {
                    event: "check_client_path",
                    path: path,
                    reply_action: actionInfo ? actionInfo.action : null,
                    reply_context: actionInfo ? actionInfo.context : null
                }
            }));
        }
    }
};

function openDashboard() {
    var url = "dashboard/index.html?lang=" + piLang;
    dashboardWindow = window.open(url, "YMDashboard", "width=920,height=640,left=160,top=120");
}
window.openDashboard = openDashboard;
