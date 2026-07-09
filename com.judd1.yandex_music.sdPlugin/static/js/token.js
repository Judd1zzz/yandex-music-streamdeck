var TOKEN_I18N = {
    en: {
        title: "Authentication",
        ph_token: "Paste your token here...",
        btn_save: "Update token",
        footer: "Validating the token requires the local API server (api_for_plugin) to be running",
        checking: "Checking...",
        saved: "✅ Token is valid, saved!",
        handoff_failed: "⚠️ Could not hand the token over — reopen the settings and try again",
        invalid: "❌ Invalid token",
        timeout: "⚠️ Server is not responding (timeout)",
        server_down: "⚠️ Local API server is not running — start api_for_plugin (localhost:8000)"
    },
    ru: {
        title: "Аутентификация",
        ph_token: "Вставьте токен сюда...",
        btn_save: "Обновить токен",
        footer: "Для проверки токена нужен запущенный локальный API-сервер (api_for_plugin)",
        checking: "Проверяю...",
        saved: "✅ Токен валидный, сохранён!",
        handoff_failed: "⚠️ Не удалось передать токен — переоткройте настройки и попробуйте снова",
        invalid: "❌ Неверный токен",
        timeout: "⚠️ Сервер не отвечает (таймаут)",
        server_down: "⚠️ Локальный API-сервер не запущен — запустите api_for_plugin (localhost:8000)"
    }
};
window.TOKEN_I18N = TOKEN_I18N;

var tokenLang = (function() {
    try {
        var lang = new URLSearchParams(window.location.search).get("lang");
        return lang === "ru" ? "ru" : "en";
    } catch (e) {
        return "en";
    }
})();

function tt(key) {
    var dict = TOKEN_I18N[tokenLang] || TOKEN_I18N.en;
    return dict[key] != null ? dict[key] : TOKEN_I18N.en[key];
}

(function applyTokenLanguage() {
    document.documentElement.lang = tokenLang;
    document.title = tokenLang === "ru" ? "Настройка токена Яндекс Музыки" : "Yandex Music Token Setup";
    document.querySelectorAll("[data-i18n]").forEach(function(el) {
        el.textContent = tt(el.getAttribute("data-i18n"));
    });
    document.querySelectorAll("[data-i18n-placeholder]").forEach(function(el) {
        el.setAttribute("placeholder", tt(el.getAttribute("data-i18n-placeholder")));
    });
})();

async function save() {
    const token = document.getElementById('token').value.trim();
    if(!token) return;

    const statusEl = document.getElementById('status_msg');
    const btn = document.getElementById('saveBtn');

    statusEl.textContent = tt("checking");
    statusEl.style.color = "#FFBD00";
    btn.disabled = true;
    btn.style.opacity = "0.7";

    const unlock = () => {
        btn.disabled = false;
        btn.style.opacity = "1";
    };

    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 10000);

    try {
        const resp = await fetch(`http://localhost:8000/check_token`, {
            method: 'GET',
            headers: { 'Authorization': token },
            signal: controller.signal
        });
        const data = await resp.json();

        if (data.valid) {
            if(window.opener && window.opener.updateToken) {
                window.opener.updateToken(token);
                statusEl.textContent = tt("saved");
                statusEl.style.color = "#4caf50";
                setTimeout(() => window.close(), 1000);
            } else {
                statusEl.textContent = tt("handoff_failed");
                statusEl.style.color = "orange";
                unlock();
            }
        } else {
            statusEl.textContent = tt("invalid");
            statusEl.style.color = "#f44336";
            unlock();
        }
    } catch (e) {
        console.error(e);
        if (e && e.name === "AbortError") {
            statusEl.textContent = tt("timeout");
        } else {
            statusEl.textContent = tt("server_down");
        }
        statusEl.style.color = "orange";
        unlock();
    } finally {
        clearTimeout(timer);
    }
}
