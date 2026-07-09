var websocket = null;
var uuid = null;
var actionInfo = null;
var pluginUUID = null;
var settings = {};
var globalSettings = {};
var globalsReady = false;
var toastTimer = null;
var piLang = "en";
var lastLocalStatus = null;
var lastTokenStatus = null;
var lastLocalReason = "";
var lastUpdateVersion = "";
var lastPathCheck = null;

var I18N = {
    en: {
        control_mode_label: "Control mode",
        opt_mode_local: "Local (this device)",
        opt_mode_ynison: "Ynison (remote)",
        btn_apply_all: "Apply to all",
        port_label: "Port",
        client_label: "Client",
        autofix_label: "Launch/relaunch the client with the debug port",
        client_path_label: "Client path",
        ph_client_path: "Empty — auto-detect",
        autofix_hint: "The plugin finds the Yandex Music client on its own and relaunches it with the required flag when needed. Set the path only if the client is installed in a non-standard location.",
        info_elements_label: "Displayed elements",
        show_cover_label: "Cover art",
        show_title_label: "Track title",
        show_artist_label: "Artist name",
        btn_style_label: "Button style",
        opt_style_v1: "original (with background)",
        progress_format_label: "Display format",
        knob_press_label: "Knob press",
        opt_knob_mute: "Mute (toggle sound)",
        knob_step_label: "Step per tick, %",
        account_label: "Account",
        btn_update_token: "Update token",
        token_info: "Click to refresh the authorization session",
        discord_rpc_label: "Rich Presence (current track in your profile)",
        ph_discord_app: "Empty — built-in application",
        discord_hint: "Leave empty to use the built-in application. A custom ID is only needed if you want a custom name/icon.",
        download_dir_label: "Download folder",
        ph_download_path: "default: Music",
        format_label: "Format",
        lang_label: "Language",
        settings_note: "Settings are saved immediately",
        ynison_title: "⚠️ Ynison — a mode for enthusiasts",
        ynison_li1: "requires a separately running local API server (api_for_plugin) and a token;",
        ynison_li2: "Yandex blocks some commands for desktop — track switching may not work;",
        ynison_li3: "in this plugin version Ynison support is limited (the Python version has it in full).",
        btn_cancel: "Cancel",
        btn_ynison_confirm: "I understand, enable",
        ynison_link: "How to run api_for_plugin — guide on GitHub",
        toast_token_saved: "Token saved",
        toast_applied_all: "Mode applied to all buttons",
        toast_no_connection: "No connection to the plugin",
        mode_desc_local: "Controls the client on this computer",
        mode_desc_ynison: "Remote control via the ynison protocol (beta)",
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
        reason_port_busy: "Port {port} is taken by another application — set a different port in the settings",
        reason_client_elevated: "The client is running as administrator — the plugin cannot control it. Untick “Run this program as an administrator” in the client shortcut properties",
        reason_elevation_declined: "The administrator rights request was declined — press any plugin button to try again",
        reason_client_not_found: "Yandex Music client not found — install it from music.yandex.ru/download or set the path in the settings"
    },
    ru: {
        control_mode_label: "Тип управления",
        opt_mode_local: "Local (это устройство)",
        opt_mode_ynison: "Ynison (удаленно)",
        btn_apply_all: "Применить ко всем",
        port_label: "Порт",
        client_label: "Клиент",
        autofix_label: "Запускать/перезапускать клиент с портом отладки",
        client_path_label: "Путь к клиенту",
        ph_client_path: "Пусто — автоопределение",
        autofix_hint: "Плагин сам находит клиент Яндекс Музыки и при необходимости перезапускает его с нужным флагом. Путь указывайте, только если клиент установлен в нестандартное место.",
        info_elements_label: "Отображаемые элементы",
        show_cover_label: "Обложка",
        show_title_label: "Название трека",
        show_artist_label: "Имя исполнителя",
        btn_style_label: "Стиль кнопки",
        opt_style_v1: "original (с фоном)",
        progress_format_label: "Формат отображения",
        knob_press_label: "Нажатие крутилки",
        opt_knob_mute: "Mute (выкл/вкл звук)",
        knob_step_label: "Шаг за тик, %",
        account_label: "Аккаунт",
        btn_update_token: "Обновить токен",
        token_info: "Нажмите, чтобы обновить сессию авторизации",
        discord_rpc_label: "Rich Presence (текущий трек в профиле)",
        ph_discord_app: "Пусто — встроенное приложение",
        discord_hint: "Оставьте пустым — используется встроенное приложение. Свой ID нужен, только если хотите отдельное имя/иконку.",
        download_dir_label: "Папка скачивания",
        ph_download_path: "по умолчанию: Музыка",
        format_label: "Формат",
        lang_label: "Язык",
        settings_note: "Настройки сохраняются сразу",
        ynison_title: "⚠️ Ynison — режим для энтузиастов",
        ynison_li1: "нужен отдельно запущенный локальный API-сервер (api_for_plugin) и токен;",
        ynison_li2: "Яндекс блокирует часть команд для ПК — переключение треков может не работать;",
        ynison_li3: "в этой версии плагина Ynison работает ограниченно (полноценно — в Python-версии).",
        btn_cancel: "Отмена",
        btn_ynison_confirm: "Я осознаю, включить",
        ynison_link: "Как запустить api_for_plugin — инструкция на GitHub",
        toast_token_saved: "Токен сохранён",
        toast_applied_all: "Режим применён ко всем кнопкам",
        toast_no_connection: "Нет связи с плагином",
        mode_desc_local: "Управление клиентом на этом компьютере",
        mode_desc_ynison: "Удаленное управление через протокол ynison (бета)",
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
        reason_port_busy: "Порт {port} занят другим приложением — укажите другой порт в настройках",
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
    updateModeUI(settings.control_mode);
    if (lastLocalStatus) updateStatusIndicator("local_status_indicator", lastLocalStatus);
    if (lastTokenStatus) updateStatusIndicator("token_status_indicator", lastTokenStatus);
    updateLocalReason(lastLocalReason);
    if (lastUpdateVersion) renderUpdateNotice(lastUpdateVersion);
    if (lastPathCheck) renderClientPathCheck(lastPathCheck);
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

function systemPrefersRussian() {
    var langs = navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language || ""];
    return langs.some(function(l) { return /^ru/i.test(String(l || "")); });
}

function hideLangOffer() {
    var el = document.getElementById("lang_offer");
    if (el) el.classList.add("hidden");
}

function chooseLanguage(lang) {
    globalSettings.pi_language = lang;
    saveGlobalSettings();
    applyLanguage(lang);
    syncGlobalSelect("pi_language", "pi_language_items", "pi_language_selected");
    hideLangOffer();
}

function maybeOfferRussian() {
    if (globalSettings.pi_language) { hideLangOffer(); return; }
    if (!systemPrefersRussian()) return;
    var el = document.getElementById("lang_offer");
    if (!el) return;
    el.textContent = "🌐 Панель настроек доступна на русском. Переключить? ";
    var yes = document.createElement("span");
    yes.className = "pi-lang-link";
    yes.id = "lang_offer_yes";
    yes.textContent = "Переключить на русский";
    yes.addEventListener("click", function() { chooseLanguage("ru"); });
    var no = document.createElement("span");
    no.className = "pi-lang-link";
    no.id = "lang_offer_no";
    no.textContent = "Оставить English";
    no.addEventListener("click", function() { chooseLanguage("en"); });
    el.appendChild(yes);
    el.appendChild(no);
    el.classList.remove("hidden");
}

function showToast(text) {
    const el = document.getElementById("pi_toast");
    if (!el) return;
    el.textContent = text;
    el.classList.remove("hidden");
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(function() { el.classList.add("hidden"); }, 2500);
}

function openTokenPopup() {
    const url = 'token.html?lang=' + piLang;
    const name = 'YandexTokenPopup';
    const specs = 'width=500,height=650,left=200,top=200';
    window.open(url, name, specs);
}

window.updateToken = function(newToken) {
    globalSettings.token = newToken;
    saveGlobalSettings();
    showToast(t("toast_token_saved"));
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
    requestClientPathCheck();
}

var clientPathCheckTimer = null;

function scheduleClientPathCheck() {
    if (clientPathCheckTimer) clearTimeout(clientPathCheckTimer);
    clientPathCheckTimer = setTimeout(requestClientPathCheck, 400);
}

function requestClientPathCheck() {
    const input = document.getElementById("client_path_input");
    const el = document.getElementById("client_path_check");
    if (!input || !el) return;
    const path = input.value.trim();
    if (!path) {
        el.textContent = "";
        el.className = "pi-hint pi-path-check hidden";
        return;
    }
    if (!websocket || websocket.readyState !== 1) return;
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

function renderClientPathCheck(payload) {
    const el = document.getElementById("client_path_check");
    if (!el) return;
    lastPathCheck = payload;
    const expected = payload.expected || "";
    let text = "";
    let tone = "warn";
    if (payload.verdict === "ok") {
        text = t("path_ok");
        tone = "ok";
    } else if (payload.verdict === "ok_dir") {
        text = t("path_ok_dir", { file: payload.resolved || expected });
        tone = "warn";
    } else if (payload.verdict === "missing") {
        text = t("path_missing");
        tone = "err";
    } else if (payload.verdict === "dir_without_client") {
        text = t("path_no_client", { file: expected });
        tone = "err";
    }
    if (!text) {
        el.textContent = "";
        el.className = "pi-hint pi-path-check hidden";
        return;
    }
    el.textContent = text;
    el.className = "pi-hint pi-path-check pi-path-" + tone;
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
        showToast(t("toast_applied_all"));
    } else {
        showToast(t("toast_no_connection"));
    }
}

function updateModeUI(mode) {
    const desc = document.getElementById("mode_description");
    const localGroup = document.getElementById("local_settings_group");
    const tokenBlock = document.getElementById("token_settings_block");
    
    const cleanMode = (mode || "local").toString().trim().toLowerCase();
    
    if (cleanMode === 'local') {
        if(desc) desc.textContent = t("mode_desc_local");
        if(localGroup) localGroup.classList.remove("hidden");
        if(tokenBlock) tokenBlock.classList.add("hidden");
    } else {
        if(desc) desc.textContent = t("mode_desc_ynison");
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

(function initYnisonRepoLink() {
    var link = document.getElementById("ynison_repo_link");
    if (!link) return;
    link.addEventListener("click", function() {
        openExternal(piLang === "ru"
            ? "https://github.com/Judd1zzz/yandex-music-streamdeck/blob/main/README_RU.md#режим-ynison-экспериментальный"
            : "https://github.com/Judd1zzz/yandex-music-streamdeck#ynison-mode-experimental");
    });
})();

function initGlobalSelect(selectedId, itemsId, globalKey, callback) {
    initSelect(selectedId, itemsId, function(val) {
        globalSettings[globalKey] = val;
        saveGlobalSettings();
    }, callback);
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
        initGlobalSelect("pi_language_selected", "pi_language_items", "pi_language", function(val) {
            applyLanguage(val);
            hideLangOffer();
        });

        const rawAction = String(actionInfo.action || '');
        const action = rawAction.replace(/_/g, '-');
        if (rawAction.indexOf('.yandex-music.') !== -1) {
            const dlBlock = document.getElementById('download_global_block');
            if (dlBlock) dlBlock.classList.add('hidden');
        }
        if (action === "com.judd1.yandex-music.action.info") {
            document.getElementById('info_settings').classList.remove('hidden');
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
            if (clientPath && document.activeElement !== clientPath) scheduleClientPathCheck();
            const discordChk = document.getElementById("chk_discord_rpc");
            if (discordChk) discordChk.checked = globalSettings.discord_rpc_enabled === true;
            const discordApp = document.getElementById("discord_app_id_input");
            if (discordApp && globalSettings.discord_app_id) discordApp.value = globalSettings.discord_app_id;
            const dlPath = document.getElementById("download_path_input");
            if (dlPath && typeof globalSettings.download_path === "string") dlPath.value = globalSettings.download_path;
            syncGlobalSelect("download_format", "download_format_items", "download_format_selected");
            applyLanguage(globalSettings.pi_language === "ru" ? "ru" : "en");
            syncGlobalSelect("pi_language", "pi_language_items", "pi_language_selected");
            maybeOfferRussian();
        } else if (jsonObj.event === 'sendToPropertyInspector') {
            var payload = jsonObj.payload;
            if (payload.event === "TokenStatus") {
                lastTokenStatus = payload.status;
                updateStatusIndicator("token_status_indicator", payload.status);
                updateLocalReason("");
            } else if (payload.event === "LocalStatus") {
                 lastLocalStatus = payload.status;
                 updateStatusIndicator("local_status_indicator", payload.status);
                 updateLocalReason(payload.status === "connected" ? "" : payload.reason);
            } else if (payload.event === "ClientPathCheck") {
                renderClientPathCheck(payload);
            } else if (payload.event === "UpdateNotice") {
                renderUpdateNotice(payload.version);
            }
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
        const portInput = document.getElementById("local_port_input");
        const port = globalSettings.local_port || (portInput && portInput.value) || 9222;
        return t("reason_" + raw, { port: port });
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
