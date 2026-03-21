/**
 * DWI — Digital Waste Index
 * Frontend logic — polls Rust backend every 5s for system metrics,
 * carbon impact, waste alerts, wallet, and daily report.
 */

const { invoke } = window.__TAURI__.core;

// ─── Helpers ───

function animateValue(element, target, decimals = 1) {
    const current = parseFloat(element.textContent) || 0;
    const diff = target - current;
    if (Math.abs(diff) < 0.05) {
        element.textContent = target.toFixed(decimals);
        return;
    }
    const steps = 12;
    let step = 0;
    function tick() {
        step++;
        const ease = 1 - Math.pow(1 - step / steps, 3);
        element.textContent = (current + diff * ease).toFixed(decimals);
        if (step < steps) requestAnimationFrame(tick);
    }
    requestAnimationFrame(tick);
}

function formatBytes(bytes) {
    if (bytes < 1024) return { value: bytes.toFixed(0), unit: 'B' };
    if (bytes < 1048576) return { value: (bytes / 1024).toFixed(1), unit: 'KB' };
    if (bytes < 1073741824) return { value: (bytes / 1048576).toFixed(1), unit: 'MB' };
    return { value: (bytes / 1073741824).toFixed(2), unit: 'GB' };
}

function formatRate(kbps) {
    if (kbps < 1) return { value: (kbps * 1024).toFixed(0), unit: 'B/s' };
    if (kbps < 1024) return { value: kbps.toFixed(1), unit: 'KB/s' };
    return { value: (kbps / 1024).toFixed(1), unit: 'MB/s' };
}

function formatUptime(seconds) {
    const d = Math.floor(seconds / 86400);
    const h = Math.floor((seconds % 86400) / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    if (d > 0) return `${d}d ${h}h ${m}m`;
    if (h > 0) return `${h}h ${m}m`;
    return `${m}m`;
}

function formatDuration(secs) {
    const m = Math.floor(secs / 60);
    const s = secs % 60;
    if (m > 60) return `${Math.floor(m / 60)}h ${m % 60}m`;
    return `${m}m ${s}s`;
}

function formatFileSize(bytes) {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1048576) return (bytes / 1024).toFixed(1) + ' KB';
    if (bytes < 1073741824) return (bytes / 1048576).toFixed(1) + ' MB';
    return (bytes / 1073741824).toFixed(2) + ' GB';
}

function getSeverity(grams) {
    if (grams < 20) return 'low';
    if (grams < 100) return 'mid';
    return 'high';
}

function getContext(carbon) {
    if (carbon.total_co2_grams < 0.1) return 'Gathering baseline metrics.';
    if (carbon.equiv_phone_charges >= 0.01)
        return `≈ charging a phone ${carbon.equiv_phone_charges.toFixed(2)}×`;
    if (carbon.equiv_led_minutes >= 1)
        return `≈ ${carbon.equiv_led_minutes.toFixed(0)} min of a 10W LED bulb`;
    return 'Minimal footprint — keep it up!';
}

function actionIcon(type) {
    const icons = {
        'tab_close': '🗂️', 'idle_kill': '💤', 'duplicate_delete': '🗑️',
        'manual': '✋', 'doomscroll_fix': '📡', 'tab_tax_fix': '🧹',
    };
    return icons[type] || '⚡';
}

// ─── State ───
let isFirstPoll = true;
let lastKnownBalance = 0;
let currentAlert = null;

// ─── Main Poll ───

async function pollMetrics() {
    try {
        const s = await invoke('get_system_status');

        // Telemetry
        animateValue(document.getElementById('cpu-value'), s.cpu_usage, 1);
        animateValue(document.getElementById('ram-value'), s.ram_used_mb, 0);
        document.getElementById('ram-detail').textContent =
            `${Math.round(s.ram_used_mb).toLocaleString()} / ${Math.round(s.ram_total_mb).toLocaleString()} MB`;

        const rx = formatRate(s.net_rx_rate_kbps);
        animateValue(document.getElementById('net-rx-value'), parseFloat(rx.value), rx.unit === 'B/s' ? 0 : 1);
        document.getElementById('net-rx-unit').textContent = rx.unit;

        const tx = formatRate(s.net_tx_rate_kbps);
        animateValue(document.getElementById('net-tx-value'), parseFloat(tx.value), tx.unit === 'B/s' ? 0 : 1);
        document.getElementById('net-tx-unit').textContent = tx.unit;

        document.getElementById('uptime-value').textContent = formatUptime(s.uptime_seconds);

        const rxTotal = formatBytes(s.net_rx_bytes);
        document.getElementById('total-rx').textContent = `${rxTotal.value} ${rxTotal.unit}`;
        const txTotal = formatBytes(s.net_tx_bytes);
        document.getElementById('total-tx').textContent = `${txTotal.value} ${txTotal.unit}`;

        const time = s.timestamp.split('T')[1] || s.timestamp;
        document.getElementById('last-update').textContent = `Last update: ${time}`;

        if (isFirstPoll) {
            document.getElementById('daemon-status').textContent = 'Monitoring';
            isFirstPoll = false;
        }

        // ─── Hero Score ───
        const c = s.carbon;
        animateValue(document.getElementById('dwi-score-val'), c.total_co2_grams, 2);
        document.getElementById('hero-session-time').textContent = formatDuration(c.session_seconds);

        const sev = getSeverity(c.total_co2_grams);
        document.getElementById('dwi-score-val').className = 'score-number severity-' + sev;

        document.getElementById('score-context').textContent = getContext(c);
        document.getElementById('carbon-cpu').textContent = c.cpu_co2_grams.toFixed(2) + 'g';
        document.getElementById('carbon-net').textContent = c.net_co2_grams.toFixed(2) + 'g';

        // ─── Alert Banner ───
        const alertBanner = document.getElementById('alert-banner');
        if (s.active_alert) {
            currentAlert = s.active_alert;
            alertBanner.style.display = 'flex';
            document.getElementById('alert-msg').textContent = s.active_alert.message;
            document.getElementById('alert-credits').textContent = s.active_alert.recoverable_co2.toFixed(1);
        } else {
            alertBanner.style.display = 'none';
            currentAlert = null;
        }

    } catch (err) {
        console.warn('[DWI] Poll error:', err);
        document.getElementById('daemon-status').textContent = 'Warming Up...';
    }

    try {
        const count = await invoke('get_metric_count');
        document.getElementById('snapshot-count').textContent = count.toLocaleString();
    } catch (_) {}
}

// ─── Wallet Poll ───

async function pollWallet() {
    try {
        const st = await invoke('get_wallet_status');

        const creditsEl = document.getElementById('credits-val');
        if (st.balance > lastKnownBalance && lastKnownBalance > 0) {
            creditsEl.classList.add('sparkle');
            setTimeout(() => creditsEl.classList.remove('sparkle'), 400);
        }
        lastKnownBalance = st.balance;

        animateValue(creditsEl, st.balance, 1);
        document.getElementById('credits-today').textContent = st.today_credits.toFixed(1);
        document.getElementById('credits-lifetime').textContent = st.lifetime_total.toFixed(1);
        document.getElementById('streak-val').textContent = st.streak;

        const badge = document.getElementById('streak-badge');
        if (st.streak > 0) { badge.classList.add('active'); } else { badge.classList.remove('active'); }

        const historyList = document.getElementById('history-list');
        if (st.recent_actions.length === 0) {
            historyList.innerHTML = '<li class="empty-state">System optimized. Awaiting new data.</li>';
        } else {
            historyList.innerHTML = st.recent_actions.map(a => `
                <li>
                    <span><span class="action-icon">${actionIcon(a.action_type)}</span>${a.description || a.action_type}</span>
                    <span class="credit-gain">${a.credits > 0 ? '+' : ''}${a.credits.toFixed(1)} EC</span>
                </li>
            `).join('');
        }

        // Redeem button visibility
        const redeemBtn = document.getElementById('redeem-portal-btn');
        if (st.balance >= 5) {
            redeemBtn.style.display = 'block';
            redeemBtn.textContent = '🎁 Rewards Store';
            redeemBtn.classList.remove('progress');
        } else if (st.balance >= 3) {
            redeemBtn.style.display = 'block';
            redeemBtn.textContent = `${((st.balance / 5) * 100).toFixed(0)}% to Rewards`;
            redeemBtn.classList.add('progress');
        } else {
            redeemBtn.style.display = 'none';
        }

    } catch (e) {
        console.warn('[DWI] Wallet poll error:', e);
    }
}

// ─── Redeem Portal ───

document.getElementById('redeem-portal-btn').addEventListener('click', () => {
    window.location.href = 'redeem.html';
});

// ─── Alert Action ───

document.getElementById('resolve-alert-btn').addEventListener('click', async () => {
    if (!currentAlert) return;
    const btn = document.getElementById('resolve-alert-btn');
    btn.disabled = true;
    btn.textContent = 'Awarding...';

    try {
        const actionType = currentAlert.alert_type === 'tab_tax' ? 'tab_tax_fix' : 'doomscroll_fix';
        const desc = currentAlert.alert_type === 'tab_tax'
            ? `Resolved Tab Tax: ${currentAlert.message}`
            : 'Resolved Doomscroll alert';
        await invoke('award_eco_credits', {
            co2Grams: currentAlert.recoverable_co2,
            actionType, description: desc,
        });
        document.getElementById('alert-banner').style.display = 'none';
        currentAlert = null;
        await pollWallet();
    } catch (err) {
        console.error('[DWI] Alert resolve failed:', err);
    }

    btn.disabled = false;
    btn.innerHTML = 'Fix & Earn <span id="alert-credits">0</span> EC';
});

// ─── Simulate Button ───

document.getElementById('test-award-btn').addEventListener('click', async () => {
    const btn = document.getElementById('test-award-btn');
    btn.disabled = true;
    btn.textContent = 'Awarding...';

    try {
        await invoke('award_eco_credits', {
            co2Grams: 5.0,
            actionType: 'tab_close',
            description: 'Closed 5 idle tabs',
        });
        await pollWallet();
    } catch (err) {
        console.error('[DWI] Award failed:', err);
    }

    btn.disabled = false;
    btn.textContent = '+ Simulate Tab Close';
});

// ─── Cloud Cleaner ───

document.getElementById('scan-btn').addEventListener('click', async () => {
    const pathInput = document.getElementById('scan-path');
    const btn = document.getElementById('scan-btn');
    const scanStatus = document.getElementById('scan-status');
    let path = pathInput.value.trim();

    if (!path) { scanStatus.textContent = 'Please enter a directory path.'; return; }

    btn.disabled = true;
    btn.textContent = 'Scanning...';
    scanStatus.textContent = 'Scanning directory...';
    document.getElementById('scan-results').style.display = 'none';

    try {
        const result = await invoke('scan_duplicates', { path });

        document.getElementById('scan-total-files').textContent = result.total_files.toLocaleString();
        document.getElementById('scan-dup-groups').textContent = result.duplicate_groups;
        document.getElementById('scan-wasted').textContent = formatFileSize(result.total_wasted_bytes);
        document.getElementById('scan-co2').textContent = result.wasted_co2_grams.toFixed(2) + 'g';

        const dupList = document.getElementById('dup-list');
        if (result.groups.length === 0) {
            dupList.innerHTML = '<li class="empty-state">✅ No duplicates found!</li>';
        } else {
            dupList.innerHTML = result.groups.slice(0, 20).map(g => {
                const basename = g.paths[0].split('/').pop();
                return `
                    <li class="dup-item">
                        <span><strong>${basename}</strong> × ${g.paths.length} copies</span>
                        <span class="dup-item-waste">${formatFileSize(g.wasted_bytes)} wasted</span>
                    </li>`;
            }).join('');
        }

        document.getElementById('scan-results').style.display = 'block';
        scanStatus.textContent = `Scanned ${result.scanned_dir} — ${result.total_files} files.`;
    } catch (err) {
        scanStatus.textContent = '❌ ' + err;
        console.error('[DWI] Scan error:', err);
    }

    btn.disabled = false;
    btn.textContent = 'Scan';
});

// ─── End-of-Day Report ───

async function pollDailyReport() {
    try {
        const r = await invoke('get_daily_report');
        document.getElementById('eod-date').textContent = r.date;
        document.getElementById('eod-co2-generated').textContent = r.session_co2_grams.toFixed(2);
        document.getElementById('eod-co2-prevented').textContent = r.co2_prevented_grams.toFixed(2);
        document.getElementById('eod-credits').textContent = r.credits_earned.toFixed(1);
        document.getElementById('eod-actions').textContent = r.actions_taken;
        document.getElementById('eod-avg-cpu').textContent = r.avg_cpu_usage.toFixed(1);
        document.getElementById('eod-snapshots').textContent = r.snapshots_collected.toLocaleString();
        document.getElementById('eod-streak').textContent = r.current_streak;
    } catch (e) {
        console.warn('[DWI] Daily report error:', e);
    }
}

document.getElementById('eod-refresh-btn').addEventListener('click', pollDailyReport);

// ─── Init ───

setTimeout(() => {
    pollMetrics();
    pollWallet();
    pollDailyReport();
}, 1500);

setInterval(() => {
    pollMetrics();
    pollWallet();
}, 5000);

setInterval(pollDailyReport, 30000);
