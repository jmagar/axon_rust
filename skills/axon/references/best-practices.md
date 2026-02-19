# Axon Best Practices

Essential patterns for efficient Axon usage.

---

## File Organization

**CRITICAL:** Always organize Axon output properly to avoid cluttering the workspace and to enable .gitignore.

### Directory Structure

Create a `.axon/` folder in the working directory to store results:

```bash
# Create organization directories
mkdir -p .axon/{scratchpad,docs,research,news}

# Add to .gitignore
echo ".axon/" >> .gitignore
```

**Directory purposes:**
- `.axon/` - All Axon output (gitignored)
- `.axon/scratchpad/` - Temporary scripts and one-time operations
- `.axon/docs/` - Documentation scraping
- `.axon/research/` - Research projects
- `.axon/news/` - News and articles

### Output File Naming

Always use `-o` to write directly to file (avoids flooding context):

```bash
# Search results
axon search "your query" -o .axon/search-{query}.json

# Search with scraping
axon search "your query" --scrape -o .axon/search-{query}-scraped.json

# Scrape pages
axon scrape https://example.com -o .axon/{site}-{path}.md
```

**Naming patterns:**
```
.axon/search-react_server_components.json
.axon/search-ai_news-scraped.json
.axon/docs.github.com-actions-overview.md
.axon/firecrawl.dev.md
```

**For organized projects:**
```bash
# Competitor research
.axon/competitor-research/company-a.md
.axon/competitor-research/company-b.md

# Documentation by project
.axon/docs/nextjs/routing.md
.axon/docs/nextjs/api.md

# Time-based news
.axon/news/2024-01/ai-trends.json
.axon/news/2024-01/tech-releases.json
```

**Temporary scripts:**
```bash
# One-time processing scripts
.axon/scratchpad/bulk-scrape.sh
.axon/scratchpad/process-results.sh
```

### URL Quoting in Shell Commands

**IMPORTANT:** Always quote URLs in shell commands - `?` and `&` are special characters in all shell environments (bash, zsh, sh, fish, etc.):

```bash
# CORRECT - Works in all shells
axon scrape "https://example.com/page?query=test&limit=10" -o .axon/example.md

# WRONG - Shell interprets ? and & as special characters
axon scrape https://example.com/page?query=test&limit=10 -o .axon/example.md
```

This applies to:
- Query parameters (`?key=value`)
- Multiple parameters (`&key=value`)
- Anchors (`#section`)
- Any URL with special shell characters

---

## Reading Output Files

**CRITICAL: NEVER read entire Axon output files at once unless explicitly asked or required.**

Axon output files are often 1000+ lines. Reading entire files wastes context window tokens and slows down processing.

### Best Practices

**1. Check file size first:**
```bash
# Check line count and preview structure
wc -l .axon/file.md && head -50 .axon/file.md

# Check file size in human-readable format
ls -lh .axon/file.md
```

**2. Use grep to find specific content:**
```bash
# Search for keywords
grep -n "keyword" .axon/file.md

# Show context around matches (10 lines before/after)
grep -A 10 -B 10 "## Section" .axon/file.md

# Case-insensitive search
grep -i "api endpoint" .axon/file.md

# Multiple patterns
grep -E "pattern1|pattern2" .axon/file.md
```

**3. Read incrementally with offset/limit:**
```bash
# Using Read tool with offset and limit
Read(file=".axon/file.md", offset=1, limit=100)      # First 100 lines
Read(file=".axon/file.md", offset=100, limit=100)    # Next 100 lines
Read(file=".axon/file.md", offset=200, limit=100)    # Lines 200-300
```

**4. Use head/tail for previews:**
```bash
# First 50 lines
head -50 .axon/file.md

# Last 50 lines
tail -50 .axon/file.md

# Lines 100-200
head -200 .axon/file.md | tail -100
```

**5. Process with other bash commands:**
```bash
# Extract specific sections with awk
awk '/^## Section/,/^## Next Section/' .axon/file.md

# Count occurrences
grep -c "keyword" .axon/file.md

# Extract URLs only
grep -oP 'https?://[^\s]+' .axon/file.md

# Get unique domains
grep -oP 'https?://[^/]+' .axon/file.md | sort -u

# Process JSON with jq
jq '.data.web[].title' .axon/search-results.json
```

**6. Dynamic sizing based on file:**
```bash
# Determine appropriate read size
file_lines=$(wc -l < .axon/file.md)

if [ "$file_lines" -lt 100 ]; then
    # Small file - read all
    cat .axon/file.md
elif [ "$file_lines" -lt 500 ]; then
    # Medium file - read first 200 lines
    head -200 .axon/file.md
else
    # Large file - grep for keywords or read incrementally
    grep -n "important_keyword" .axon/file.md
fi
```

### When Full Reading is Acceptable

**ONLY read entire files when:**
- User explicitly requests full file reading
- File is confirmed to be <100 lines
- Processing requires complete content (rare)
- Building summaries of complete documents

**Default approach:** Always start with grep, head, or incremental reading. Only escalate to full file reading if absolutely necessary.

---

## Parallelization

**CRITICAL: ALWAYS run multiple scrapes in parallel, never sequentially.**

Sequential scraping is dramatically slower. Always check concurrency limits and run up to that many jobs simultaneously.

### Check Concurrency Limits

```bash
# Check current concurrency and credits
axon --status
```

**Example output:**
```
axon cli v1.0.2

● Authenticated via FIRECRAWL_API_KEY
Concurrency: 0/100 jobs (parallel scrape limit)
Credits: 500,000 remaining
```

The concurrency limit (e.g., 100) indicates how many jobs can run in parallel.

### Parallel Execution Patterns

#### Bad: Sequential Execution (NEVER DO THIS)

```bash
# WRONG - Sequential execution is 3x-10x slower
axon scrape https://site1.com -o .axon/1.md
axon scrape https://site2.com -o .axon/2.md
axon scrape https://site3.com -o .axon/3.md
```

#### Good: Background Jobs with & and wait

```bash
# CORRECT - Parallel execution with background jobs
axon scrape https://site1.com -o .axon/1.md &
axon scrape https://site2.com -o .axon/2.md &
axon scrape https://site3.com -o .axon/3.md &
wait  # Wait for all background jobs to complete

echo "All scrapes completed"
```

#### Best: xargs for Bulk Operations

For many URLs, use `xargs` with `-P` (parallel) flag:

```bash
# Parallel scraping with xargs (10 concurrent jobs)
cat urls.txt | xargs -P 10 -I {} sh -c '
    url="{}"
    filename=$(echo "$url" | md5sum | cut -d" " -f1)
    axon scrape "$url" -o ".axon/${filename}.md"
'
```

**Advanced xargs patterns:**

```bash
# Scrape with domain-based organization
cat urls.txt | xargs -P 10 -I {} sh -c '
    url="{}"
    domain=$(echo "$url" | sed -E "s|https?://([^/]+).*|\1|")
    filename=$(echo "$url" | md5sum | cut -d" " -f1)
    mkdir -p ".axon/${domain}"
    axon scrape "$url" -o ".axon/${domain}/${filename}.md"
'

# Search and scrape results in parallel
axon search "AI research" --limit 20 --json | \
    jq -r '.data.web[].url' | \
    xargs -P 10 -I {} sh -c '
        url="{}"
        filename=$(echo "$url" | tr "/:?&#" "_____")
        axon scrape "$url" -o ".axon/research-${filename}.md"
    '
```

### Parallelization Guidelines

**How many parallel jobs?**
- Check `axon --status` for concurrency limit
- Run close to limit but not over (e.g., 95/100)
- For `xargs -P`, use 10-20 for most cases
- For `&` and `wait`, batch in groups of 10-50

**Error handling in parallel execution:**
```bash
# Capture failures and continue
for url in $(cat urls.txt); do
    (
        if ! axon scrape "$url" -o ".axon/$(echo $url | md5sum | cut -d' ' -f1).md"; then
            echo "FAILED: $url" >> .axon/failures.log
        fi
    ) &

    # Limit concurrent jobs to 10
    if (( $(jobs -r | wc -l) >= 10 )); then
        # wait -n requires bash >= 4.3; fall back to wait (all) on older versions
        if [[ "${BASH_VERSINFO[0]}" -ge 5 || ( "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 3 ) ]]; then
            wait -n
        else
            wait
        fi
    fi
done
wait  # Wait for remaining jobs
```

**Performance comparison:**
- Sequential: 10 URLs = 50-100 seconds
- Parallel (10 concurrent): 10 URLs = 5-10 seconds
- **10x faster with parallelization**

**When NOT to parallelize:**
- Single URL scraping
- Testing/debugging operations
- Rate-limited sources (add `--delay`)
- When concurrency limit is reached

---

## Security Patterns

### Input Sanitization

Always sanitize user input before using in commands, URLs, or API calls:

```bash
# Sanitize user input - remove dangerous characters
sanitize_input() {
    local input="$1"
    # Remove shell metacharacters and command injection attempts
    echo "$input" | sed 's/[;&|`$(){}[\]<>\\]//g' | tr -d '\n\r'
}

# Usage
user_query=$(sanitize_input "$1")
```

### Command Injection Prevention

Never directly interpolate user input into shell commands or URLs:

```bash
# ❌ DANGEROUS - Command injection vulnerability
curl "https://api.example.com/search?q=$user_input"

# ✅ SAFE - Properly escaped and quoted
query=$(printf '%s' "$user_input" | jq -sRr @uri)
curl "https://api.example.com/search?q=${query}"
```

### URL Encoding

Always URL-encode user input when building API requests:

```bash
# URL encode function
url_encode() {
    local string="$1"
    printf '%s' "$string" | jq -sRr @uri
}

# Usage
search_term=$(url_encode "user's search & query")
curl "https://api.example.com/search?q=${search_term}"
```
