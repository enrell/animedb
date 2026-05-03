import re

with open('crates/animedb/src/provider.rs', 'r') as f:
    content = f.read()

helper = """
    fn execute_with_retry(&self, payload: &serde_json::Value) -> Result<reqwest::blocking::Response> {
        let mut retry_count = 0;
        let mut delay = std::time::Duration::from_secs(2);
        loop {
            let req = self.client.post(&self.endpoint).json(payload).build()?;
            let resp = self.client.execute(req)?;
            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                if retry_count >= 3 {
                    return Ok(resp.error_for_status()?);
                }
                if let Some(retry_after) = resp.headers().get(reqwest::header::RETRY_AFTER) {
                    if let Ok(s) = retry_after.to_str() {
                        if let Ok(secs) = s.parse::<u64>() {
                            delay = std::time::Duration::from_secs(secs + 1);
                        }
                    }
                }
                std::thread::sleep(delay);
                retry_count += 1;
                delay *= 2;
                continue;
            }
            return Ok(resp.error_for_status()?);
        }
    }
"""

split_target = "provider.endpoint = endpoint.into();\n        provider\n    }"

parts = content.split(split_target)

# We want to add the helper after the FIRST occurrence of split_target.
# The delimiter should be put back for all splits.
res = parts[0] + split_target + "\n" + helper
for part in parts[1:]:
    if part == parts[-1]:
        res += part
    else:
        res += part + split_target

content = res

content = re.sub(
    r'let response = self\s*\.client\s*\.post\(&self\.endpoint\)\s*\.json\(&payload\)\s*\.send\(\)\?\s*\.error_for_status\(\)\?',
    r'let response = self.execute_with_retry(&payload)?',
    content
)

with open('crates/animedb/src/provider.rs', 'w') as f:
    f.write(content)
