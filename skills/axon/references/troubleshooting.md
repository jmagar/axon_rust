# Axon Troubleshooting Guide

Common issues and solutions for Axon CLI and API.

## Authentication Errors

### 401 Unauthorized

**Error:**
```
Error: 401 Unauthorized - Invalid or missing API key
```

**Causes:**
- Invalid API key
- API key not set in environment
- Expired API key
- Wrong API endpoint

**Solutions:**

1. Check API key in `.env` file:
```bash
cat ~/.claude-homelab/.env | grep FIRECRAWL_API_KEY
```

2. Verify API key is correct:
```bash
# Get new key from https://firecrawl.dev/
# Update .env file
echo 'FIRECRAWL_API_KEY="fc-your-new-key"' >> ~/.claude-homelab/.env
```

3. Test authentication:
```bash
axon --status
```

4. Re-login if needed:
```bash
axon logout
axon login --api-key fc-YOUR-KEY
```

---

### API Key Required

**Error:**
```
Error: API key required for cloud API
```

**Causes:**
- `FIRECRAWL_API_KEY` not set
- Using cloud API without credentials

**Solutions:**

1. Add API key to `.env`:
```bash
echo 'FIRECRAWL_API_KEY="fc-your-api-key"' >> ~/.claude-homelab/.env
```

2. Or provide via flag:
```bash
axon https://example.com --api-key fc-YOUR-KEY
```

3. Or use self-hosted instance (no key required):
```bash
echo 'FIRECRAWL_API_URL="http://localhost:3002"' >> ~/.claude-homelab/.env
echo 'FIRECRAWL_API_KEY=""' >> ~/.claude-homelab/.env
```

---

## Connection Errors

### Connection Refused

**Error:**
```
Error: connect ECONNREFUSED 127.0.0.1:3002
```

**Causes:**
- Self-hosted Firecrawl not running
- Wrong API URL
- Network issues

**Solutions:**

1. Check if service is running (self-hosted):
```bash
docker ps | grep firecrawl
# Or
curl http://localhost:3002/health
```

2. Verify API URL in `.env`:
```bash
cat ~/.claude-homelab/.env | grep FIRECRAWL_API_URL
```

3. Start service if stopped:
```bash
docker compose up -d firecrawl
```

4. Check firewall rules:
```bash
sudo ufw status
```

---

### Timeout Errors

**Error:**
```
Error: Request timeout after 30000ms
```

**Causes:**
- Website not responding
- Slow JavaScript rendering
- Network latency
- Rate limiting

**Solutions:**

1. Increase wait time for JavaScript sites:
```bash
axon https://example.com --wait-for 10000
```

2. Check website manually:
```bash
curl -I https://example.com
```

3. Try with reduced concurrency:
```bash
axon crawl https://example.com --max-concurrency 1 --wait
```

4. Add delay between requests:
```bash
axon crawl https://example.com --delay 2000 --wait
```

---

## Rate Limiting

### 429 Too Many Requests

**Error:**
```
Error: 429 Too Many Requests - Rate limit exceeded
```

**Causes:**
- Too many requests in short time
- Concurrency limit reached
- Server protection triggered

**Solutions:**

1. Add delay between requests:
```bash
axon crawl https://example.com --delay 1000 --wait
```

2. Reduce concurrency:
```bash
axon crawl https://example.com --max-concurrency 2 --wait
```

3. Check rate limit status:
```bash
axon --status
```

4. Wait before retrying:
```bash
sleep 60
axon https://example.com
```

---

### Concurrency Limit Reached

**Error:**
```
Error: Concurrency limit reached (100/100)
```

**Causes:**
- Too many jobs running simultaneously
- Previous jobs not completed

**Solutions:**

1. Check current status:
```bash
axon --status
```

2. Wait for existing jobs to complete
3. Use async mode and poll later:
```bash
# Start crawl (returns job ID)
axon crawl https://example.com --limit 50

# Check status later
axon --status
```

4. Cancel running jobs if needed (via web UI)

---

## Data Extraction Issues

### No Content Extracted

**Error:**
```
Warning: No content extracted from page
```

**Causes:**
- JavaScript-rendered content not loaded
- Page requires authentication
- Content behind paywall
- Anti-scraping protection

**Solutions:**

1. Wait for JavaScript to render:
```bash
axon https://example.com --wait-for 5000
```

2. Check if content is main article:
```bash
axon https://example.com --only-main-content
```

3. Try different format:
```bash
axon https://example.com --format html
```

4. Verify manually in browser:
```bash
curl https://example.com | grep "expected content"
```

---

### Too Much Clutter

**Error:**
```
Output includes navigation, ads, footers
```

**Causes:**
- Not using main content filter
- Page structure unclear
- Multiple content sections

**Solutions:**

1. Use main content filter:
```bash
axon https://example.com --only-main-content
```

2. Exclude specific tags:
```bash
axon https://example.com --exclude-tags "nav,footer,aside,script,style"
```

3. Include only content tags:
```bash
axon https://example.com --include-tags "article,main,section,p,h1,h2,h3"
```

4. Use markdown format for cleaner output:
```bash
axon https://example.com --format markdown --only-main-content
```

---

### Missing Specific Elements

**Error:**
```
Expected elements not in output
```

**Causes:**
- Tag filtering too strict
- Elements loaded by JavaScript
- Elements in excluded sections

**Solutions:**

1. Check tag filters:
```bash
# Remove exclude-tags if used
axon https://example.com
```

2. Wait for dynamic content:
```bash
axon https://example.com --wait-for 5000
```

3. Use HTML format to preserve structure:
```bash
axon https://example.com --format html
```

4. Try raw HTML:
```bash
axon https://example.com --format rawHtml
```

---

## Crawl Issues

### Crawl Stopped Early

**Error:**
```
Crawl completed with fewer pages than expected
```

**Causes:**
- Hit page limit
- Reached max depth
- No more links to follow
- Path filtering too restrictive

**Solutions:**

1. Increase page limit:
```bash
axon crawl https://example.com --limit 500 --wait
```

2. Increase max depth:
```bash
axon crawl https://example.com --max-depth 5 --wait
```

3. Check path filters:
```bash
# Remove or adjust include-paths
axon crawl https://example.com --wait
```

4. Enable entire domain:
```bash
axon crawl https://example.com --crawl-entire-domain --wait
```

---

### Crawl Too Slow

**Error:**
```
Crawl taking too long to complete
```

**Causes:**
- Rate limiting active
- Low concurrency
- Large delay between requests

**Solutions:**

1. Reduce delay:
```bash
axon crawl https://example.com --delay 500 --wait
```

2. Increase concurrency:
```bash
axon crawl https://example.com --max-concurrency 10 --wait
```

3. Use async mode (non-blocking):
```bash
# Start crawl (returns job ID)
axon crawl https://example.com --limit 100

# Check progress later
axon --status
```

4. Limit scope:
```bash
axon crawl https://example.com --limit 100 --max-depth 2 --wait
```

---

### Paths Not Included

**Error:**
```
Expected paths missing from crawl
```

**Causes:**
- Path filtering incorrect
- Pattern syntax wrong
- Paths not linked from start URL

**Solutions:**

1. Check include-paths pattern:
```bash
# Use wildcards correctly
axon crawl https://example.com --include-paths "/blog/*" --wait
```

2. Use multiple patterns:
```bash
axon crawl https://example.com --include-paths "/blog/*,/docs/*" --wait
```

3. Map site first to see structure:
```bash
axon map https://example.com --search "blog"
```

4. Remove path filters:
```bash
axon crawl https://example.com --wait
```

---

## Output Issues

### Invalid JSON

**Error:**
```
SyntaxError: Unexpected token in JSON
```

**Causes:**
- Mixing single and multiple formats
- Piping errors with output
- Incomplete response

**Solutions:**

1. Use pretty-print for JSON:
```bash
axon https://example.com --format markdown,html --pretty
```

2. Save to file instead of stdout:
```bash
axon https://example.com --format markdown,html -o output.json
```

3. Redirect errors separately:
```bash
axon https://example.com 2> errors.log
```

4. Validate JSON with jq:
```bash
axon https://example.com --format markdown,html | jq .
```

---

### File Save Failed

**Error:**
```
Error: ENOENT: no such file or directory
```

**Causes:**
- Output directory doesn't exist
- No write permissions
- Invalid file path

**Solutions:**

1. Create output directory:
```bash
mkdir -p output/
axon https://example.com -o output/page.md
```

2. Check permissions:
```bash
ls -la output/
chmod 755 output/
```

3. Use absolute path:
```bash
axon https://example.com -o /home/user/output/page.md
```

4. Test with temp directory:
```bash
axon https://example.com -o /tmp/test.md
```

---

## Credit and Quota Issues

### Out of Credits

**Error:**
```
Error: Insufficient credits
```

**Causes:**
- Account credits depleted
- Free tier limit reached

**Solutions:**

1. Check credit usage:
```bash
axon credit-usage
```

2. Check account status:
```bash
axon --status
```

3. Upgrade plan or add credits (via web UI)

4. Use self-hosted instance:
```bash
echo 'FIRECRAWL_API_URL="http://localhost:3002"' >> ~/.claude-homelab/.env
```

---

### Quota Exceeded

**Error:**
```
Error: Monthly quota exceeded
```

**Causes:**
- Used all monthly quota
- Plan limit reached

**Solutions:**

1. Wait for quota reset:
```bash
axon credit-usage
# Check resetDate
```

2. Upgrade plan (via web UI)

3. Use self-hosted for unlimited quota

4. Optimize requests:
```bash
# Use main content only
axon https://example.com --only-main-content

# Reduce crawl scope
axon crawl https://example.com --limit 50 --max-depth 2 --wait
```

---

## Environment Configuration

### .env File Not Found

**Error:**
```
ERROR: .env file not found at ~/.claude-homelab/.env
```

**Causes:**
- `.env` file doesn't exist
- Wrong file location
- Incorrect permissions

**Solutions:**

1. Create `.env` file:
```bash
touch ~/.claude-homelab/.env
chmod 600 ~/.claude-homelab/.env
```

2. Add credentials:
```bash
cat >> ~/.claude-homelab/.env <<EOF
FIRECRAWL_API_KEY="fc-your-api-key"
FIRECRAWL_API_URL="https://api.firecrawl.dev"
EOF
```

3. Verify file exists:
```bash
ls -la ~/.claude-homelab/.env
```

4. Check file permissions:
```bash
chmod 600 ~/.claude-homelab/.env
```

---

### Environment Variables Not Loaded

**Error:**
```
Using credentials from .env but getting auth errors
```

**Causes:**
- Variables not exported
- Wrong variable names
- Syntax errors in `.env`

**Solutions:**

1. Check `.env` syntax:
```bash
cat ~/.claude-homelab/.env
# Look for syntax errors (quotes, spacing)
```

2. Source manually to test:
```bash
set -a
source ~/.claude-homelab/.env
set +a
echo $FIRECRAWL_API_KEY
```

3. Verify variable names:
```bash
grep FIRECRAWL ~/.claude-homelab/.env
# Should be FIRECRAWL_API_KEY and FIRECRAWL_API_URL
```

4. Use direct flag as workaround:
```bash
axon https://example.com --api-key fc-YOUR-KEY
```

---

## Performance Issues

### Memory Errors

**Error:**
```
out of memory / OOM killed / memory allocation failed
```

**Causes:**
- Large crawl results exceeding available RAM
- Too many concurrent requests
- System OOM killer terminating the process

**Solutions:**

1. Limit crawl scope:
```bash
axon crawl https://example.com --limit 100 --max-depth 2 --wait
```

2. Reduce batch concurrency:
```bash
axon crawl https://example.com --batch-concurrency 4 --wait
```

3. Check available system memory:
```bash
free -h
```

4. Increase process memory limits (Linux):
```bash
# Check current limits
ulimit -v

# Increase virtual memory limit for current session
ulimit -v unlimited
```

5. Process in batches to reduce peak memory:
```bash
# Map first to get URLs
axon map https://example.com -o urls.json

# Scrape URLs in smaller batches
axon batch --urls "$(jq -r '.urls[:50] | join(",")' urls.json)" --wait
```

---

### Disk Space Full

**Error:**
```
Error: ENOSPC: no space left on device
```

**Causes:**
- Large output files
- Multiple crawls saved
- Disk full

**Solutions:**

1. Check disk space:
```bash
df -h
```

2. Clean up old files:
```bash
rm -rf ~/claude-homelab/skills/axon/output/*
```

3. Use streaming output:
```bash
axon https://example.com | gzip > output.md.gz
```

4. Save to different mount:
```bash
axon https://example.com -o /mnt/storage/output.md
```

---

## Common Command Issues

### Command Not Found

**Error:**
```
bash: axon: command not found
```

**Causes:**
- `axon` binary not built or not in PATH
- Binary not compiled yet

**Solutions:**

1. Check if `axon` is already available:
```bash
which axon
```

2. Build from source:
```bash
cd /path/to/axon_rust
cargo build --release --bin axon
```

3. Add to PATH:
```bash
export PATH="$PATH:/path/to/axon_rust/target/release"
```

4. Or create a symlink:
```bash
sudo ln -sf /path/to/axon_rust/target/release/axon /usr/local/bin/axon
```

---

### Permission Denied

**Error:**
```
bash: ./scripts/scrape.sh: Permission denied
```

**Causes:**
- Script not executable
- Wrong file permissions

**Solutions:**

1. Make script executable:
```bash
chmod +x scripts/scrape.sh
```

2. Make all scripts executable:
```bash
chmod +x scripts/*.sh
```

3. Run with bash explicitly:
```bash
bash scripts/scrape.sh https://example.com
```

---

## Getting Help

### Check Logs

```bash
# Enable debug output
DEBUG=* axon https://example.com

# Save output and errors
axon https://example.com > output.log 2>&1
```

### Test Configuration

```bash
# Check status
axon --status

# Test authentication
axon login --api-key fc-YOUR-KEY

# View configuration
axon config
```

### Contact Support

If issues persist:

1. Check [official documentation](https://docs.firecrawl.dev/)
2. Visit [GitHub issues](https://github.com/firecrawl/cli/issues)
3. Join [Discord community](https://discord.gg/firecrawl)
4. Contact support (cloud users)

---

## Additional Resources

- [Official Documentation](https://docs.firecrawl.dev/)
- [CLI Documentation](https://docs.firecrawl.dev/sdks/cli)
- [GitHub Repository](https://github.com/firecrawl/firecrawl)
- [CLI GitHub](https://github.com/firecrawl/cli)
- [API Endpoints Reference](./api-endpoints.md)
- [Quick Reference Guide](./quick-reference.md)
