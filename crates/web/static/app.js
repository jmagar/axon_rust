// Axon Web UI — WebSocket-powered command execution + Docker stats

(function () {
    'use strict';

    // ── DOM refs ──
    const omnibox       = document.getElementById('omnibox');
    const urlInput      = document.getElementById('urlInput');
    const actionLabel   = document.getElementById('actionLabel');
    const actionArrow   = document.getElementById('actionArrow');
    const modeIcon      = document.getElementById('modeIcon');
    const modeName      = document.getElementById('modeName');
    const modeDropdown  = document.getElementById('modeDropdown');
    const resultsPanel      = document.getElementById('resultsPanel');
    const resultsBody       = document.getElementById('resultsBody');
    const resultsStats      = document.getElementById('resultsStats');
    const commandOptions    = document.getElementById('commandOptions');
    const dockerStats       = document.getElementById('dockerStats');
    const resultsRecent     = document.getElementById('resultsRecent');
    const resultTabs        = document.getElementById('resultTabs');
    const omniboxStatus     = document.getElementById('omniboxStatus');
    const statusDotInline   = document.getElementById('statusDotInline');
    const statusTextInline  = document.getElementById('statusTextInline');
    const interfaceCard     = document.querySelector('.interface-card');
    const container         = document.querySelector('.container');
    const wsIndicator       = document.getElementById('wsIndicator');
    const wsLabel           = document.getElementById('wsLabel');
    const modeOptions       = modeDropdown.querySelectorAll('.mode-option');

    let currentMode = 'scrape';
    let currentView = 'markdown';   // 'markdown' | 'html'
    let isProcessing = false;
    let hasExecuted = false;
    let cachedHtmlOutput = '';       // accumulated raw HTML when view=html

    // Modes that require no input — auto-execute on selection
    const NO_INPUT_MODES = new Set([
        'stats', 'status', 'doctor', 'domains', 'sources', 'suggest', 'debug', 'sessions'
    ]);

    // Commands that support view mode switching (Markdown / HTML)
    const VIEW_MODE_COMMANDS = new Set(['scrape', 'crawl', 'map']);

    const viewModes = document.getElementById('viewModes');
    const viewModeButtons = viewModes.querySelectorAll('.view-mode');

    // ── Recent runs (persisted in memory) ──
    const recentRuns = [];

    // ── Tab switching ──
    resultTabs.addEventListener('click', (e) => {
        const tab = e.target.closest('.result-tab');
        if (!tab) return;
        const pane = tab.dataset.pane;
        resultTabs.querySelectorAll('.result-tab').forEach(t => t.classList.remove('active'));
        tab.classList.add('active');
        document.querySelectorAll('.result-pane').forEach(p => p.classList.remove('active'));
        document.getElementById('pane' + pane.charAt(0).toUpperCase() + pane.slice(1)).classList.add('active');
    });

    // ── View mode switching (Markdown / HTML) ──
    viewModeButtons.forEach(function (btn) {
        btn.addEventListener('click', function () {
            var view = btn.dataset.view;
            if (view === currentView) return;
            currentView = view;
            viewModeButtons.forEach(function (b) { b.classList.remove('active'); });
            btn.classList.add('active');

            // If switching view on a completed result, re-execute with new format
            if (!isProcessing && hasExecuted && urlInput.value.trim()) {
                execute();
            }
        });
    });

    // ── Options text filter state machine ──
    // Detects plain-text "◐ Scraping..." + "Options:" + key/value lines from stdout
    // and routes them to the Stats tab's commandOptions div instead of Content.
    var optionsCapture = false;
    var optionsCapturedLines = [];

    function isOptionsHeaderLine(line) {
        // Matches "  ◐ Scraping https://..." or "  Options:"
        return /^\s*◐\s/.test(line) || /^\s*Options:\s*$/.test(line);
    }

    function isOptionsKvLine(line) {
        // Matches "  format: Markdown" or "  renderMode: auto-switch" style lines
        return /^\s{2,}\w[\w\-]*:\s/.test(line);
    }

    function flushCapturedOptions() {
        if (optionsCapturedLines.length === 0) return;
        var section = document.createElement('div');
        section.style.marginBottom = '16px';
        var heading = document.createElement('h3');
        heading.style.cssText = 'color: #8787af; font-size: 11px; text-transform: uppercase; letter-spacing: 1px; margin-bottom: 8px; font-family: "DM Sans", sans-serif;';
        heading.textContent = 'Command Options';
        section.appendChild(heading);

        var card = document.createElement('div');
        card.className = 'result-card';
        var html = '<div class="kv-table">';
        optionsCapturedLines.forEach(function (kvLine) {
            var match = kvLine.match(/^\s*(\w[\w\-]*):\s*(.*)/);
            if (match) {
                var label = match[1].replace(/_/g, ' ').replace(/([a-z])([A-Z])/g, '$1 $2');
                var val = match[2] || '—';
                html += '<div class="kv-row"><span class="kv-key">' + escapeHtml(label) + '</span><span class="kv-value">' + escapeHtml(val) + '</span></div>';
            }
        });
        html += '</div>';
        card.innerHTML = html;
        section.appendChild(card);
        commandOptions.appendChild(section);
        optionsCapturedLines = [];
    }

    // ── Mode selection ──
    actionArrow.addEventListener('click', (e) => {
        e.stopPropagation();
        omnibox.classList.toggle('dropdown-open');
    });

    actionLabel.addEventListener('click', (e) => {
        e.stopPropagation();
        execute();
    });

    modeOptions.forEach(option => {
        option.addEventListener('click', (e) => {
            e.stopPropagation();
            modeOptions.forEach(o => o.classList.remove('active'));
            option.classList.add('active');
            currentMode = option.dataset.mode;
            modeName.textContent = option.textContent.trim();
            const iconPath = option.dataset.icon;
            modeIcon.innerHTML = '<path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="' + iconPath + '"/>';
            omnibox.classList.remove('dropdown-open');

            if (NO_INPUT_MODES.has(currentMode)) {
                // Auto-execute — no input needed
                execute();
            } else {
                urlInput.focus();
            }
        });
    });

    document.addEventListener('click', (e) => {
        if (!omnibox.contains(e.target)) omnibox.classList.remove('dropdown-open');
    });
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape') omnibox.classList.remove('dropdown-open');
    });

    urlInput.addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            e.preventDefault();
            execute();
        }
    });

    // ── WebSocket connection ──
    const MAX_BACKOFF = 30000;
    const BASE_BACKOFF = 1000;
    let ws = null;
    let reconnectAttempts = 0;
    let reconnectTimer = null;

    // Neuron cluster assignment for Docker stats
    const containerNeuronMap = new Map();
    let assignedContainers = [];

    function assignNeuronClusters(containerNames) {
        containerNeuronMap.clear();
        assignedContainers = containerNames.sort();
        if (assignedContainers.length === 0 || !window.neurons) return;
        const perContainer = Math.floor(window.neurons.length / assignedContainers.length);
        assignedContainers.forEach((name, idx) => {
            const start = idx * perContainer;
            const end = idx === assignedContainers.length - 1 ? window.neurons.length : start + perContainer;
            containerNeuronMap.set(name, { start, end });
        });
    }

    function setWsStatus(state, label) {
        wsIndicator.className = 'ws-indicator ' + state;
        wsLabel.textContent = label;
    }

    function connectWs() {
        if (ws && (ws.readyState === WebSocket.CONNECTING || ws.readyState === WebSocket.OPEN)) {
            return;
        }

        const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = proto + '//' + location.host + '/ws';

        try {
            ws = new WebSocket(wsUrl);
        } catch (e) {
            scheduleReconnect();
            return;
        }

        ws.onopen = function () {
            reconnectAttempts = 0;
            setWsStatus('connected', 'CONNECTED');
        };

        ws.onmessage = function (event) {
            try {
                const msg = JSON.parse(event.data);
                handleMessage(msg);
            } catch (e) {
                // malformed message
            }
        };

        ws.onclose = function () {
            setWsStatus('reconnecting', 'RECONNECTING');
            scheduleReconnect();
        };

        ws.onerror = function () {
            // onclose fires after
        };
    }

    function scheduleReconnect() {
        if (reconnectTimer) return;
        const delay = Math.min(BASE_BACKOFF * Math.pow(2, reconnectAttempts), MAX_BACKOFF);
        reconnectAttempts++;
        setWsStatus('reconnecting', 'RETRY ' + Math.round(delay / 1000) + 's');
        reconnectTimer = setTimeout(() => {
            reconnectTimer = null;
            connectWs();
        }, delay);
    }

    // ── Message dispatcher ──
    // Accumulated output lines for the current command
    let outputLines = [];
    let commandStartTime = 0;

    function handleMessage(msg) {
        switch (msg.type) {
            case 'output':
                handleOutput(msg);
                break;
            case 'stdout_json':
                // Structured JSON from sync commands — render as rich content
                if (msg.data) renderJsonOutput(msg.data);
                break;
            case 'stdout_line':
                // Plain text from sync commands
                if (msg.line) appendRawLine(msg.line);
                break;
            case 'command_start':
                // Mode announced — no visual action needed
                break;
            case 'screenshot_files':
                handleScreenshotFiles(msg);
                break;
            case 'log':
                handleLog(msg);
                break;
            case 'done':
                handleDone(msg);
                break;
            case 'error':
                handleError(msg);
                break;
            case 'stats':
                handleStats(msg);
                break;
        }
    }

    function handleLog(msg) {
        // Stderr progress lines — show as subtle status updates
        var line = (msg.line || '').trim();
        if (!line) return;

        // Update the inline status text with the latest log line
        statusTextInline.innerHTML = '<span class="spinner"></span> ' + escapeHtml(line);

        // Also append to the results body as a log line
        var div = document.createElement('div');
        div.className = 'log-line';
        div.textContent = line;
        resultsBody.appendChild(div);
        resultsBody.scrollTop = resultsBody.scrollHeight;
    }

    function handleOutput(msg) {
        outputLines.push(msg.line);

        // ── Options text filter ──
        // Detect the plain-text "◐ Scraping..." / "Options:" / "  key: value" block
        var line = msg.line;
        if (!optionsCapture && isOptionsHeaderLine(line)) {
            optionsCapture = true;
            if (/^\s*◐\s/.test(line)) {
                // "◐ Scraping https://..." — capture as a status header, not content
                optionsCapturedLines = [];
            }
            return;  // suppress from Content
        }
        if (optionsCapture) {
            if (isOptionsKvLine(line)) {
                optionsCapturedLines.push(line);
                return;  // suppress from Content
            }
            if (/^\s*Options:\s*$/.test(line)) {
                return;  // suppress the "Options:" header itself
            }
            // Non-matching line or blank — end of options block
            optionsCapture = false;
            flushCapturedOptions();
            // If blank line right after options block, suppress it too
            if (!line.trim()) return;
        }

        // ── HTML view mode: accumulate raw output for iframe ──
        if (currentView === 'html') {
            // Skip non-HTML header lines ("Scrape Results for...", "As of:...")
            if (/^Scrape Results for\s/.test(line) || /^As of:\s/.test(line)) {
                return;
            }
            cachedHtmlOutput += line + '\n';
            // Don't render line-by-line — will show iframe on 'done'
            return;
        }

        // Try to parse JSON output lines and render rich content
        var parsed = null;
        try {
            parsed = JSON.parse(msg.line);
        } catch (e) {
            // plain text line
        }

        if (parsed) {
            renderJsonOutput(parsed);
        } else {
            appendRawLine(msg.line);
        }
    }

    // Keys that indicate an object is config/options metadata, not content
    var CONFIG_KEYS = new Set([
        'format', 'renderMode', 'render_mode', 'proxy', 'userAgent', 'user_agent',
        'timeoutMs', 'timeout_ms', 'fetchRetries', 'fetch_retries', 'retryBackoffMs',
        'retry_backoff_ms', 'chromeAntiBot', 'chrome_anti_bot', 'chromeStealth',
        'chrome_stealth', 'chromeIntercept', 'chrome_intercept', 'embed',
        'requestTimeout', 'request_timeout', 'concurrency', 'maxPages', 'max_pages',
        'maxDepth', 'max_depth', 'includeSubdomains', 'include_subdomains',
        'discoverSitemaps', 'discover_sitemaps', 'performanceProfile', 'performance_profile',
        'delay', 'delayMs', 'delay_ms'
    ]);

    function isConfigObject(obj) {
        if (typeof obj !== 'object' || obj === null || Array.isArray(obj)) return false;
        var keys = Object.keys(obj);
        if (keys.length < 2) return false;
        var configHits = 0;
        for (var i = 0; i < keys.length; i++) {
            if (CONFIG_KEYS.has(keys[i])) configHits++;
        }
        return configHits >= 2;
    }

    // Render screenshot images inline when screenshot_files message arrives
    function handleScreenshotFiles(msg) {
        var files = msg.files || [];
        if (files.length === 0) return;

        // Clear any previously rendered JSON output for screenshots
        // (the stdout_json with {url, path, size_bytes} is metadata — the image is what matters)
        resultsBody.innerHTML = '';

        files.forEach(function (file) {
            var wrapper = document.createElement('div');
            wrapper.style.marginBottom = '16px';

            // Image
            var imgContainer = document.createElement('div');
            imgContainer.style.cssText = 'border-radius: 8px; overflow: hidden; border: 1px solid rgba(175,215,255,0.1); background: rgba(10,18,35,0.4);';
            var img = document.createElement('img');
            img.src = file.serve_url || ('/output/screenshots/' + file.name);
            img.alt = 'Screenshot: ' + file.name;
            img.style.cssText = 'width: 100%; display: block;';
            img.loading = 'lazy';
            imgContainer.appendChild(img);
            wrapper.appendChild(imgContainer);

            // Metadata bar
            var meta = document.createElement('div');
            meta.style.cssText = 'display: flex; gap: 12px; align-items: center; margin-top: 8px; font-size: 11px; color: #5f6b7a; font-family: "JetBrains Mono", monospace;';
            meta.innerHTML = '<span>' + escapeHtml(file.name) + '</span>';
            if (file.size_bytes) {
                var kb = (file.size_bytes / 1024).toFixed(1);
                meta.innerHTML += '<span>' + kb + ' KB</span>';
            }
            meta.innerHTML += '<a href="' + escapeHtml(file.serve_url || ('/output/screenshots/' + file.name)) + '" download="' + escapeHtml(file.name) + '" style="margin-left: auto; color: #87afff;">Download</a>';
            wrapper.appendChild(meta);

            resultsBody.appendChild(wrapper);
        });
        resultsBody.scrollTop = resultsBody.scrollHeight;
    }

    // Extract and render markdown-rich content from JSON output
    function renderJsonOutput(obj) {
        // Screenshot metadata ({url, path, size_bytes}) — suppress, image shown via screenshot_files
        if (obj && obj.path && obj.size_bytes !== undefined && obj.url && currentMode === 'screenshot') {
            return;
        }

        // Config/options metadata → route to Stats tab
        if (isConfigObject(obj)) {
            var section = document.createElement('div');
            section.style.marginBottom = '16px';
            var heading = document.createElement('h3');
            heading.style.cssText = 'color: #8787af; font-size: 11px; text-transform: uppercase; letter-spacing: 1px; margin-bottom: 8px; font-family: "DM Sans", sans-serif;';
            heading.textContent = 'Options';
            section.appendChild(heading);
            var card = document.createElement('div');
            card.className = 'result-card';
            card.innerHTML = renderObjectAsHtml(obj);
            section.appendChild(card);
            commandOptions.appendChild(section);
            return;
        }

        // scrape: { url, markdown, title, description }
        if (obj.markdown) {
            var wrapper = document.createElement('div');
            wrapper.className = 'markdown-content';
            if (obj.title) {
                var h = document.createElement('h2');
                h.textContent = obj.title;
                wrapper.appendChild(h);
            }
            if (obj.url) {
                var link = document.createElement('a');
                link.href = obj.url;
                link.target = '_blank';
                link.className = 'source-url';
                link.textContent = obj.url;
                wrapper.appendChild(link);
            }
            var content = document.createElement('div');
            content.innerHTML = parseMarkdown(obj.markdown);
            wrapper.appendChild(content);
            resultsBody.appendChild(wrapper);
            resultsBody.scrollTop = resultsBody.scrollHeight;
            return;
        }

        // ask: { query, answer }
        if (obj.answer) {
            var wrapper = document.createElement('div');
            wrapper.className = 'markdown-content';
            if (obj.query) {
                var q = document.createElement('div');
                q.className = 'ask-query';
                q.innerHTML = '<strong>Q:</strong> ' + escapeHtml(obj.query);
                wrapper.appendChild(q);
            }
            var content = document.createElement('div');
            content.innerHTML = parseMarkdown(obj.answer);
            wrapper.appendChild(content);
            resultsBody.appendChild(wrapper);
            resultsBody.scrollTop = resultsBody.scrollHeight;
            return;
        }

        // query/retrieve: { rank, score, url, snippet }
        if (obj.rank !== undefined && obj.snippet) {
            var card = document.createElement('div');
            card.className = 'result-card';
            var header = '<div class="result-rank">#' + obj.rank + '</div>';
            header += '<div class="result-score">' + (obj.score != null ? obj.score.toFixed(4) : '') + '</div>';
            if (obj.url) {
                header += '<a href="' + escapeHtml(obj.url) + '" target="_blank" class="result-url">' + escapeHtml(obj.url) + '</a>';
            }
            card.innerHTML = header + '<div class="result-snippet">' + parseMarkdown(obj.snippet) + '</div>';
            resultsBody.appendChild(card);
            resultsBody.scrollTop = resultsBody.scrollHeight;
            return;
        }

        // Generic structured output — render as clean key-value pairs
        var card = document.createElement('div');
        card.className = 'result-card';
        card.innerHTML = renderObjectAsHtml(obj);
        resultsBody.appendChild(card);
        resultsBody.scrollTop = resultsBody.scrollHeight;
    }

    // Render any JS object/array as clean human-readable HTML (no raw JSON)
    function renderObjectAsHtml(obj, depth) {
        depth = depth || 0;
        if (obj === null || obj === undefined) return '<span class="kv-null">—</span>';
        if (typeof obj === 'string') {
            // If it looks like markdown or multiline, parse it
            if (obj.indexOf('\n') !== -1 || /^#{1,6}\s/.test(obj)) {
                return '<div class="kv-text">' + parseMarkdown(obj) + '</div>';
            }
            // If it looks like a URL, make it clickable
            if (/^https?:\/\//.test(obj)) {
                return '<a href="' + escapeHtml(obj) + '" target="_blank" class="source-url">' + escapeHtml(obj) + '</a>';
            }
            return '<span class="kv-string">' + escapeHtml(obj) + '</span>';
        }
        if (typeof obj === 'number') return '<span class="kv-number">' + obj + '</span>';
        if (typeof obj === 'boolean') return '<span class="kv-bool">' + (obj ? 'yes' : 'no') + '</span>';

        if (Array.isArray(obj)) {
            if (obj.length === 0) return '<span class="kv-null">none</span>';
            // Array of simple values: inline
            if (obj.every(function(v) { return typeof v !== 'object' || v === null; })) {
                return obj.map(function(v) { return renderObjectAsHtml(v, depth + 1); }).join(', ');
            }
            // Array of objects: render each as a sub-card
            var html = '';
            obj.forEach(function(item, idx) {
                html += '<div class="kv-array-item">' + renderObjectAsHtml(item, depth + 1) + '</div>';
            });
            return html;
        }

        // Object: key-value table
        var keys = Object.keys(obj);
        if (keys.length === 0) return '<span class="kv-null">—</span>';

        var html = '<div class="kv-table">';
        keys.forEach(function(key) {
            var val = obj[key];
            var label = key.replace(/_/g, ' ').replace(/([a-z])([A-Z])/g, '$1 $2');
            html += '<div class="kv-row">';
            html += '<span class="kv-key">' + escapeHtml(label) + '</span>';
            html += '<span class="kv-value">' + renderObjectAsHtml(val, depth + 1) + '</span>';
            html += '</div>';
        });
        html += '</div>';
        return html;
    }

    function appendRawLine(line) {
        // Skip empty lines
        if (!line.trim()) return;
        var div = document.createElement('div');
        div.className = 'output-line';
        // Check if the line itself looks like markdown (starts with #, -, *, etc.)
        if (/^#{1,6}\s|^\s*[-*+]\s|^\s*\d+\.\s|^```|^>/.test(line)) {
            div.innerHTML = parseMarkdown(line);
        } else {
            div.textContent = line;
        }
        resultsBody.appendChild(div);
        resultsBody.scrollTop = resultsBody.scrollHeight;
    }

    function handleDone(msg) {
        const elapsed = msg.elapsed_ms ? (msg.elapsed_ms / 1000).toFixed(1) : '0.0';

        statusDotInline.className = 'status-dot done';
        statusTextInline.innerHTML = '<span>' + currentMode + '</span> &bull; ' + elapsed + 's &bull; exit ' + (msg.exit_code || 0);

        // Flush any remaining captured options
        if (optionsCapture) {
            optionsCapture = false;
            flushCapturedOptions();
        }

        // HTML view mode: render accumulated HTML in sandboxed iframe
        if (currentView === 'html' && cachedHtmlOutput.trim()) {
            renderHtmlPreview(cachedHtmlOutput);
        }

        // Add to recent runs
        addRecentRun('done', currentMode, urlInput.value.trim(), elapsed + 's', outputLines.length);

        finishExecution();
    }

    function renderHtmlPreview(html) {
        resultsBody.innerHTML = '';
        var iframe = document.createElement('iframe');
        iframe.className = 'html-preview-frame';
        iframe.sandbox = 'allow-same-origin';
        iframe.srcdoc = html;
        resultsBody.appendChild(iframe);
        // Auto-resize iframe once content loads
        iframe.addEventListener('load', function () {
            try {
                var h = iframe.contentDocument.documentElement.scrollHeight;
                if (h > 100) {
                    iframe.style.height = Math.min(h + 32, window.innerHeight * 0.8) + 'px';
                }
            } catch (e) {
                // cross-origin safety — keep default height
            }
        });
    }

    function handleError(msg) {
        const elapsed = msg.elapsed_ms ? (msg.elapsed_ms / 1000).toFixed(1) : '0.0';

        statusDotInline.className = 'status-dot done';
        statusTextInline.innerHTML = '<span style="color:#ff87af">' + currentMode + ' error</span> &bull; ' + elapsed + 's';

        // Show error in results
        const errDiv = document.createElement('div');
        errDiv.style.cssText = 'color: #ff87af; padding: 12px; border: 1px solid rgba(255,135,175,0.2); border-radius: 8px; margin-top: 8px;';
        errDiv.innerHTML = '<strong>Error:</strong> ' + escapeHtml(msg.message || 'Unknown error');
        if (msg.stderr) {
            errDiv.innerHTML += '<pre style="margin-top: 8px; font-size: 11px; color: #8787af;">' + escapeHtml(msg.stderr) + '</pre>';
        }
        resultsBody.appendChild(errDiv);

        addRecentRun('failed', currentMode, urlInput.value.trim(), elapsed + 's', 0);

        finishExecution();
    }

    function finishExecution() {
        isProcessing = false;
        window.isProcessing = false;

        // Decay neural intensity
        if (window.setNeuralIntensity) {
            window.setNeuralIntensity(0.15);
            setTimeout(() => {
                window.setNeuralIntensity(0);
                interfaceCard.classList.remove('firing');
                omnibox.classList.remove('firing');
            }, 3000);
        }
    }

    function handleStats(data) {
        const agg = data.aggregate || {};
        const containers = data.containers || {};
        const names = Object.keys(containers);

        // Re-assign neuron clusters if container set changed
        if (names.length !== assignedContainers.length ||
            !names.every(n => assignedContainers.includes(n))) {
            assignNeuronClusters(names);
        }

        // Map aggregate CPU to neural intensity (only when not processing a command)
        const containerCount = data.container_count || 1;
        const maxExpectedCpu = containerCount * 100;
        const cpuNorm = Math.min(agg.cpu_percent / maxExpectedCpu, 1.0);

        if (!isProcessing && window.setNeuralIntensity) {
            const bridgeIntensity = 0.02 + cpuNorm * 0.83;
            window.setNeuralIntensity(bridgeIntensity);
        }

        // Per-container neuron stimulation
        if (window.neurons) {
            for (const [name, metrics] of Object.entries(containers)) {
                const cluster = containerNeuronMap.get(name);
                if (!cluster) continue;

                const containerCpu = metrics.cpu_percent / 100;
                for (let i = cluster.start; i < cluster.end; i++) {
                    const neuron = window.neurons[i];
                    if (!neuron || neuron.isFiring || neuron.refractoryTime > 0) continue;
                    if (Math.random() < containerCpu * 0.08) {
                        neuron.epsp += 15 + containerCpu * 20;
                    }
                }

                const netRate = (metrics.net_rx_rate + metrics.net_tx_rate);
                const netIntensity = Math.min(netRate / (1024 * 1024), 1.0);
                if (netIntensity > 0.01) {
                    const extraSignals = Math.floor(netIntensity * 4);
                    for (let s = 0; s < extraSignals; s++) {
                        const idx = cluster.start + Math.floor(Math.random() * (cluster.end - cluster.start));
                        const n = window.neurons[idx];
                        if (n && !n.isFiring && n.refractoryTime <= 0) {
                            n.epsp += 30 + netIntensity * 25;
                        }
                    }
                }
            }
        }

        // Update WS indicator with stats summary
        setWsStatus('connected',
            'LIVE ' + data.container_count + '\u00d7' +
            ' CPU ' + agg.cpu_percent.toFixed(0) + '%'
        );

        // Render stats pane
        renderStatsPane(data);
    }

    // ── Execute command ──
    function execute() {
        const input = urlInput.value.trim();
        if (isProcessing) return;
        if (!input && !NO_INPUT_MODES.has(currentMode)) return;
        if (!ws || ws.readyState !== WebSocket.OPEN) {
            showError('Not connected to server. Waiting for reconnect...');
            return;
        }

        isProcessing = true;
        window.isProcessing = true;
        outputLines = [];
        cachedHtmlOutput = '';
        optionsCapture = false;
        optionsCapturedLines = [];
        commandStartTime = Date.now();

        // Show/hide view mode selector based on command type
        viewModes.style.display = VIEW_MODE_COMMANDS.has(currentMode) ? '' : 'none';

        omnibox.classList.remove('dropdown-open');

        if (!hasExecuted) {
            hasExecuted = true;
            container.classList.add('has-results');
        }

        // Fire neural network
        if (window.setNeuralIntensity) {
            window.setNeuralIntensity(1);
        }
        interfaceCard.classList.add('firing');
        omnibox.classList.add('firing');

        // Show processing state
        statusDotInline.className = 'status-dot processing';
        statusTextInline.innerHTML = '<span class="spinner"></span> processing';
        omniboxStatus.classList.add('visible');
        resultsBody.innerHTML = '';
        commandOptions.innerHTML = '';
        resultsPanel.classList.add('expanded');

        // Reset to Content tab
        resultTabs.querySelectorAll('.result-tab').forEach(t => t.classList.toggle('active', t.dataset.pane === 'content'));
        document.querySelectorAll('.result-pane').forEach(p => p.classList.remove('active'));
        document.getElementById('paneContent').classList.add('active');

        // Build flags from current mode context
        const flags = {};

        // Pass format flag based on view mode
        if (VIEW_MODE_COMMANDS.has(currentMode) && currentView === 'html') {
            flags.format = 'html';
        }

        // Send execute message over WebSocket
        ws.send(JSON.stringify({
            type: 'execute',
            mode: currentMode,
            input: input,
            flags: flags
        }));
    }

    function showError(message) {
        if (!hasExecuted) {
            hasExecuted = true;
            container.classList.add('has-results');
        }
        resultsPanel.classList.add('expanded');
        resultsBody.innerHTML = '<div style="color: #ff87af; padding: 12px;">' + escapeHtml(message) + '</div>';
    }

    // ── Render helpers ──
    function renderStatsPane(data) {
        const agg = data.aggregate || {};
        const containers = data.containers || {};
        const names = Object.keys(containers).sort();

        let html = '<div class="stats-grid">';
        html += statCard(data.container_count, 'Containers');
        html += statCard(agg.cpu_percent.toFixed(1) + '%', 'Total CPU');
        html += statCard(agg.avg_memory_percent.toFixed(1) + '%', 'Avg Memory');
        html += statCard(formatBytes(agg.total_net_io_rate) + '/s', 'Net I/O');
        html += '</div>';

        // Per-container details
        if (names.length > 0) {
            html += '<div class="log-stream">';
            names.forEach(name => {
                const m = containers[name];
                const shortName = name.replace(/^axon-/, '');
                html += '<div class="log-line">';
                html += '<span class="log-level ok">' + escapeHtml(shortName) + '</span> ';
                html += '<span class="log-msg">';
                html += 'CPU ' + m.cpu_percent.toFixed(1) + '% ';
                html += 'MEM ' + m.memory_usage_mb.toFixed(0) + 'MB/' + m.memory_limit_mb.toFixed(0) + 'MB ';
                html += 'NET \u2191' + formatBytes(m.net_tx_rate) + '/s \u2193' + formatBytes(m.net_rx_rate) + '/s';
                html += '</span>';
                html += '</div>';
            });
            html += '</div>';
        }

        dockerStats.innerHTML = html;
    }

    function statCard(value, label) {
        return '<div class="stat-card"><div class="stat-value">' + value + '</div><div class="stat-label">' + label + '</div></div>';
    }

    function formatBytes(bytes) {
        if (bytes < 1024) return bytes.toFixed(0) + 'B';
        if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + 'KB';
        return (bytes / (1024 * 1024)).toFixed(1) + 'MB';
    }

    // ── Recent runs ──
    function addRecentRun(status, mode, target, duration, lineCount) {
        recentRuns.unshift({
            status: status,
            mode: mode,
            target: target,
            duration: duration,
            lines: lineCount,
            time: new Date().toLocaleTimeString()
        });
        if (recentRuns.length > 20) recentRuns.pop();
        renderRecentPane();
    }

    function renderRecentPane() {
        if (recentRuns.length === 0) {
            resultsRecent.innerHTML = '<div style="color: #475569; padding: 24px; text-align: center;">No recent runs</div>';
            return;
        }

        let html = '<table class="recent-table"><thead><tr>';
        html += '<th></th><th>Mode</th><th>Target</th><th>Duration</th><th>Lines</th><th>Time</th>';
        html += '</tr></thead><tbody>';

        recentRuns.forEach(run => {
            html += '<tr>';
            html += '<td class="status-dot-cell"><span class="recent-dot ' + run.status + '"></span></td>';
            html += '<td><span class="recent-mode">' + escapeHtml(run.mode) + '</span></td>';
            html += '<td class="recent-url" title="' + escapeHtml(run.target) + '">' + escapeHtml(run.target) + '</td>';
            html += '<td class="recent-duration">' + escapeHtml(run.duration) + '</td>';
            html += '<td class="recent-chunks">' + run.lines + ' lines</td>';
            html += '<td class="recent-time">' + escapeHtml(run.time) + '</td>';
            html += '</tr>';
        });

        html += '</tbody></table>';
        resultsRecent.innerHTML = html;
    }

    // ── Markdown parser ──
    function parseMarkdown(md) {
        var html = '';
        var lines = md.split('\n');
        var i = 0;
        var inList = false;
        var listType = '';

        function esc(s) {
            return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
        }

        function inline(s) {
            s = s.replace(/\*\*\*(.+?)\*\*\*/g, '<strong><em>$1</em></strong>');
            s = s.replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
            s = s.replace(/\*(.+?)\*/g, '<em>$1</em>');
            s = s.replace(/`([^`]+)`/g, '<code>$1</code>');
            s = s.replace(/\[([^\]]+)\]\(([^)]+)\)/g, '<a href="$2" target="_blank">$1</a>');
            return s;
        }

        function closeList() {
            if (inList) {
                html += listType === 'ul' ? '</ul>\n' : '</ol>\n';
                inList = false;
            }
        }

        while (i < lines.length) {
            var line = lines[i];

            // Fenced code blocks
            if (line.trimStart().startsWith('```')) {
                closeList();
                i++;
                var code = '';
                while (i < lines.length && !lines[i].trimStart().startsWith('```')) {
                    code += esc(lines[i]) + '\n';
                    i++;
                }
                i++;
                html += '<pre><code>' + code + '</code></pre>\n';
                continue;
            }

            // Blank line
            if (line.trim() === '') { closeList(); i++; continue; }

            // Headers
            var hMatch = line.match(/^(#{1,6})\s+(.+)/);
            if (hMatch) {
                closeList();
                var level = hMatch[1].length;
                html += '<h' + level + '>' + inline(hMatch[2]) + '</h' + level + '>\n';
                i++; continue;
            }

            // Horizontal rule
            if (/^(-{3,}|\*{3,}|_{3,})\s*$/.test(line.trim())) {
                closeList(); html += '<hr>\n'; i++; continue;
            }

            // Table
            if (line.includes('|') && i + 1 < lines.length && /^\s*\|?\s*[-:]+/.test(lines[i + 1])) {
                closeList();
                var headers = line.split('|').map(function(c) { return c.trim(); }).filter(function(c) { return c; });
                i += 2;
                html += '<table><tr>';
                headers.forEach(function(h) { html += '<th>' + inline(h) + '</th>'; });
                html += '</tr>\n';
                while (i < lines.length && lines[i].includes('|') && lines[i].trim() !== '') {
                    var cells = lines[i].split('|').map(function(c) { return c.trim(); }).filter(function(c) { return c; });
                    html += '<tr>';
                    cells.forEach(function(c) { html += '<td>' + inline(c) + '</td>'; });
                    html += '</tr>\n';
                    i++;
                }
                html += '</table>\n';
                continue;
            }

            // Blockquote
            if (line.trimStart().startsWith('>')) {
                closeList();
                var bq = '';
                while (i < lines.length && lines[i].trimStart().startsWith('>')) {
                    bq += lines[i].replace(/^\s*>\s?/, '') + ' ';
                    i++;
                }
                html += '<blockquote>' + inline(bq.trim()) + '</blockquote>\n';
                continue;
            }

            // Unordered list
            if (/^\s*[-*+]\s/.test(line)) {
                if (!inList || listType !== 'ul') { closeList(); html += '<ul>\n'; inList = true; listType = 'ul'; }
                html += '<li>' + inline(line.replace(/^\s*[-*+]\s/, '')) + '</li>\n';
                i++; continue;
            }

            // Ordered list
            if (/^\s*\d+\.\s/.test(line)) {
                if (!inList || listType !== 'ol') { closeList(); html += '<ol>\n'; inList = true; listType = 'ol'; }
                html += '<li>' + inline(line.replace(/^\s*\d+\.\s/, '')) + '</li>\n';
                i++; continue;
            }

            // Paragraph
            closeList();
            var para = '';
            while (i < lines.length && lines[i].trim() !== '' && !/^#{1,6}\s/.test(lines[i]) && !/^\s*[-*+]\s/.test(lines[i]) && !/^\s*\d+\.\s/.test(lines[i]) && !lines[i].trimStart().startsWith('```') && !lines[i].trimStart().startsWith('>') && !/^(-{3,}|\*{3,}|_{3,})\s*$/.test(lines[i].trim()) && !(lines[i].includes('|') && i + 1 < lines.length && /^\s*\|?\s*[-:]+/.test(lines[i + 1]))) {
                para += lines[i] + ' ';
                i++;
            }
            if (para.trim()) {
                html += '<p>' + inline(para.trim()) + '</p>\n';
            }
        }
        closeList();
        return html;
    }

    // ── Utilities ──
    function escapeHtml(s) {
        if (!s) return '';
        return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
    }

    // ── Initialize ──
    renderRecentPane();
    connectWs();
})();
