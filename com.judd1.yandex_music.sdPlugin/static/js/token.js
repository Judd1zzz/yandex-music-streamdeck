async function save() {
    const token = document.getElementById('token').value.trim();
    if(!token) return;

    const statusEl = document.getElementById('status_msg');
    const btn = document.getElementById('saveBtn');

    statusEl.textContent = "Проверяю...";
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
                statusEl.textContent = "✅ Токен валидный, сохранён!";
                statusEl.style.color = "#4caf50";
                setTimeout(() => window.close(), 1000);
            } else {
                statusEl.textContent = "⚠️ Не удалось передать токен — переоткройте настройки и попробуйте снова";
                statusEl.style.color = "orange";
                unlock();
            }
        } else {
            statusEl.textContent = "❌ Неверный токен";
            statusEl.style.color = "#f44336";
            unlock();
        }
    } catch (e) {
        console.error(e);
        if (e && e.name === "AbortError") {
            statusEl.textContent = "⚠️ Сервер не отвечает (таймаут)";
        } else {
            statusEl.textContent = "⚠️ Локальный API-сервер не запущен — запустите api_for_plugin (localhost:8000)";
        }
        statusEl.style.color = "orange";
        unlock();
    } finally {
        clearTimeout(timer);
    }
}
