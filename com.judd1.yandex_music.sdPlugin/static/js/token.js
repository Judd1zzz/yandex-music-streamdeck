async function save() {
    const token = document.getElementById('token').value.trim();
    if(!token) return;
    
    const statusEl = document.getElementById('status_msg');
    const btn = document.getElementById('saveBtn');
    
    statusEl.textContent = "Проверяю...";
    statusEl.style.color = "#FFBD00";
    btn.disabled = true;
    btn.style.opacity = "0.7";
    
    try {
        const resp = await fetch(`http://localhost:8000/check_token`, {
            method: 'GET',
            headers: { 'Authorization': token }
        });
        const data = await resp.json();
        
        if (data.valid) {
            statusEl.textContent = "✅ Токен валидный!";
            statusEl.style.color = "#4caf50";
            
            if(window.opener && window.opener.updateToken) {
                window.opener.updateToken(token);
                setTimeout(() => window.close(), 1000);
            } else {
                statusEl.textContent = "Токен обновлен! (закрой окно)";
            }
        } else {
            statusEl.textContent = "❌ Неверный токен";
            statusEl.style.color = "#f44336";
            btn.disabled = false;
            btn.style.opacity = "1";
        }
    } catch (e) {
        console.error(e);
        statusEl.textContent = "⚠️ Ошибка валидации (проверь API)";
        statusEl.style.color = "orange";
        btn.disabled = false;
        btn.style.opacity = "1";
    }
}
