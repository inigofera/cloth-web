use serde::{Deserialize, Serialize};
use sqlx::{Pool, Postgres};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordReaction {
    pub emoji: DiscordEmoji,
    #[serde(default)]
    pub me: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordEmoji {
    pub name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DiscordMessage {
    pub id: String,
    pub content: String,
    pub timestamp: String,
    #[serde(default)]
    pub author: DiscordAuthor,
    #[serde(default)]
    pub reactions: Vec<DiscordReaction>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[allow(dead_code)]
pub struct DiscordAuthor {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub bot: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncResult {
    pub imported_count: usize,
    pub messages_processed: usize,
    pub new_items_created: usize,
    pub summary: Vec<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ParsedOutfit {
    pub date: String, // "YYYY-MM-DD"
    pub item_queries: Vec<String>,
    pub message_id: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ClothingCandidate {
    pub id: Uuid,
    pub name: String,
    pub brand: String,
    pub category: String,
    pub subcategory: String,
    pub color: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct BrandRef {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SubcategoryRef {
    pub id: i32,
    pub name: String,
    pub category_id: i32,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ColorRef {
    pub id: String,
}

struct PendingNewItem {
    id: Uuid,
    name: String,
    brand_id: Option<Uuid>,
    brand_name: String,
    category_id: Option<i32>,
    subcategory_id: Option<i32>,
    color_id: Option<String>,
}

impl ClothingCandidate {
    pub fn display_name(&self) -> String {
        if !self.brand.is_empty() {
            format!("{} {}", self.brand, self.name)
        } else {
            self.name.clone()
        }
    }
}

pub struct DiscordClient {
    token: String,
    channel_id: String,
    http: reqwest::Client,
}

impl DiscordClient {
    pub fn from_env() -> Option<Self> {
        let token = std::env::var("DISCORD_BOT_TOKEN").ok()?;
        let channel_id = std::env::var("DISCORD_CHANNEL_ID").ok()?;
        if token.trim().is_empty() || channel_id.trim().is_empty() {
            return None;
        }
        Some(Self {
            token: token.trim().to_string(),
            channel_id: channel_id.trim().to_string(),
            http: reqwest::Client::new(),
        })
    }

    pub async fn fetch_messages(&self, limit: u32) -> Result<Vec<DiscordMessage>, String> {
        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages?limit={}",
            self.channel_id, limit
        );

        let res = self
            .http
            .get(&url)
            .header("Authorization", format!("Bot {}", self.token))
            .header("User-Agent", "ClothWeb (1.0)")
            .send()
            .await
            .map_err(|e| format!("Network error reaching Discord: {e}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("Discord API returned {status}: {body}"));
        }

        let messages = res
            .json::<Vec<DiscordMessage>>()
            .await
            .map_err(|e| format!("Failed to parse Discord response: {e}"))?;

        Ok(messages)
    }

    pub async fn add_reaction(&self, message_id: &str, emoji: &str) -> Result<(), String> {
        let encoded_emoji = if emoji == "✅" {
            "%E2%9C%85"
        } else if emoji == "⚠️" {
            "%E2%9A%A0%EF%B8%8F"
        } else {
            emoji
        };

        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages/{}/reactions/{}/@me",
            self.channel_id, message_id, encoded_emoji
        );

        let res = self
            .http
            .put(&url)
            .header("Authorization", format!("Bot {}", self.token))
            .header("User-Agent", "ClothWeb (1.0)")
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(|e| format!("Failed to add reaction: {e}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("Discord reaction failed with {status}: {body}"));
        }

        Ok(())
    }

    pub async fn send_reply(&self, message_id: &str, content: &str) -> Result<(), String> {
        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages",
            self.channel_id
        );

        let body = serde_json::json!({
            "content": content,
            "message_reference": {
                "message_id": message_id
            }
        });

        let res = self
            .http
            .post(&url)
            .header("Authorization", format!("Bot {}", self.token))
            .header("User-Agent", "ClothWeb (1.0)")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Failed to reply on Discord: {e}"))?;

        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("Discord reply failed with {status}: {body}"));
        }

        Ok(())
    }
}

pub fn normalize_text(input: &str) -> String {
    let lower = input.to_lowercase();
    let mut cleaned = String::with_capacity(lower.len());
    for c in lower.chars() {
        if c.is_alphanumeric() {
            cleaned.push(c);
        } else {
            cleaned.push(' ');
        }
    }
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn to_title_case(input: &str) -> String {
    input
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let mut s = first.to_uppercase().to_string();
                    s.push_str(chars.as_str());
                    s
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Checks if an item query line is marked as a new item (e.g. `(new)` or `[new]`).
/// Returns `(is_new, cleaned_name)`.
pub fn extract_new_item_marker(line: &str) -> (bool, String) {
    let trimmed = line.trim();
    let lower = trimmed.to_lowercase();

    // Check (new) anywhere in the line
    if lower.contains("(new)") {
        let cleaned = trimmed
            .replace("(new)", "")
            .replace("(NEW)", "")
            .replace("(New)", "")
            .replace("(nEw)", "");
        return (true, cleaned.trim().to_string());
    }

    // Check [new] anywhere in the line
    if lower.contains("[new]") {
        let cleaned = trimmed
            .replace("[new]", "")
            .replace("[NEW]", "")
            .replace("[New]", "");
        return (true, cleaned.trim().to_string());
    }

    // Check NEW: prefix
    if lower.starts_with("new:") {
        return (true, trimmed[4..].trim().to_string());
    }

    // Check + prefix
    if trimmed.starts_with('+') {
        return (true, trimmed[1..].trim().to_string());
    }

    (false, trimmed.to_string())
}

pub fn parse_date_line(line: &str, fallback_year: i32) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parts: Vec<&str> = trimmed
        .split(|c| c == '/' || c == '-' || c == '.')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if parts.len() == 2 || parts.len() == 3 {
        let d = parts[0].parse::<u32>().ok()?;
        let m = parts[1].parse::<u32>().ok()?;
        if (1..=31).contains(&d) && (1..=12).contains(&m) {
            let y = if parts.len() == 3 {
                let raw_y = parts[2].parse::<i32>().ok()?;
                if raw_y < 100 {
                    2000 + raw_y
                } else {
                    raw_y
                }
            } else {
                fallback_year
            };
            return Some(format!("{:04}-{:02}-{:02}", y, m, d));
        }
    }
    None
}

pub fn parse_message_outfits(
    content: &str,
    msg_timestamp: &str,
    msg_id: &str,
) -> Vec<ParsedOutfit> {
    let fallback_year = if msg_timestamp.len() >= 4 {
        msg_timestamp[0..4].parse::<i32>().unwrap_or(2026)
    } else {
        2026
    };

    let mut outfits = Vec::new();
    let mut current_date: Option<String> = None;
    let mut current_items: Vec<String> = Vec::new();

    for raw_line in content.lines() {
        let mut line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('-') || line.starts_with('*') || line.starts_with('•') {
            line = line[1..].trim();
        }

        if let Some(date) = parse_date_line(line, fallback_year) {
            if let Some(prev_date) = current_date.take() {
                if !current_items.is_empty() {
                    outfits.push(ParsedOutfit {
                        date: prev_date,
                        item_queries: std::mem::take(&mut current_items),
                        message_id: msg_id.to_string(),
                    });
                }
            }
            current_date = Some(date);
        } else {
            current_items.push(line.to_string());
        }
    }

    if let Some(date) = current_date {
        if !current_items.is_empty() {
            outfits.push(ParsedOutfit {
                date,
                item_queries: current_items,
                message_id: msg_id.to_string(),
            });
        }
    } else if !current_items.is_empty() {
        let date = if msg_timestamp.len() >= 10 {
            msg_timestamp[0..10].to_string()
        } else {
            format!("{:04}-01-01", fallback_year)
        };
        outfits.push(ParsedOutfit {
            date,
            item_queries: current_items,
            message_id: msg_id.to_string(),
        });
    }

    outfits
}

pub fn score_item_match(query: &str, candidate: &ClothingCandidate) -> f64 {
    let norm_q = normalize_text(query);
    if norm_q.is_empty() {
        return 0.0;
    }

    let norm_name = normalize_text(&candidate.name);
    let norm_brand = normalize_text(&candidate.brand);
    let norm_subcat = normalize_text(&candidate.subcategory);
    let norm_cat = normalize_text(&candidate.category);
    let norm_color = normalize_text(&candidate.color);

    let full_display = format!("{norm_brand} {norm_name}").trim().to_string();
    let full_extended = format!("{norm_brand} {norm_color} {norm_subcat} {norm_name} {norm_cat}")
        .trim()
        .to_string();

    if norm_q == norm_name || norm_q == full_display {
        return 1.0;
    }

    let q_words: Vec<&str> = norm_q
        .split_whitespace()
        .filter(|w| w.len() > 1)
        .collect();

    if q_words.is_empty() {
        return 0.0;
    }

    let candidate_words: HashSet<&str> = full_extended
        .split_whitespace()
        .filter(|w| !w.is_empty())
        .collect();

    let mut matched_token_score = 0.0;
    for q_word in &q_words {
        let mut best_word_sim: f64 = 0.0;
        for c_word in &candidate_words {
            if q_word == c_word {
                best_word_sim = 1.0;
                break;
            }
            if c_word.contains(q_word) || q_word.contains(c_word) {
                best_word_sim = best_word_sim.max(0.90);
                continue;
            }
            let sim = strsim::jaro_winkler(q_word, c_word);
            if sim > best_word_sim {
                best_word_sim = sim;
            }
        }
        matched_token_score += best_word_sim;
    }
    let token_overlap = matched_token_score / (q_words.len() as f64);

    let full_sim = strsim::jaro_winkler(&norm_q, &full_display);
    let subcat_sim = strsim::jaro_winkler(&norm_q, &format!("{norm_brand} {norm_subcat}"));

    let best_str_sim = full_sim.max(subcat_sim);

    let mut score = (0.70 * token_overlap) + (0.30 * best_str_sim);

    if token_overlap >= 0.95 {
        score = score.max(0.85);
    }

    score
}

pub async fn sync_discord_for_user(
    pool: &Pool<Postgres>,
    user_id: Uuid,
) -> Result<SyncResult, String> {
    let discord = match DiscordClient::from_env() {
        Some(d) => d,
        None => {
            return Ok(SyncResult {
                imported_count: 0,
                messages_processed: 0,
                new_items_created: 0,
                summary: vec!["Discord configuration (token/channel) not set.".into()],
            });
        }
    };

    let raw_messages = discord.fetch_messages(50).await?;

    // Load active clothing items for this user to match against
    let mut candidates = sqlx::query_as::<_, ClothingCandidate>(
        r#"
        SELECT 
            ci.id, 
            ci.name, 
            COALESCE(b.name, '') as brand, 
            COALESCE(cc.name, '') as category, 
            COALESCE(cs.name, '') as subcategory, 
            COALESCE(c.id, '') as color 
        FROM "clothing-items" ci 
        LEFT JOIN brands b ON b.id = ci.brand_id 
        LEFT JOIN "clothing-categories" cc ON cc.id = ci.category_id 
        LEFT JOIN "clothing-subcategories" cs ON cs.id = ci.subcategory_id 
        LEFT JOIN colors c ON c.id = ci.color_id 
        WHERE ci.user_id = $1 AND ci.is_active = true
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("Failed to fetch clothing items: {e}"))?;

    // Load reference dictionaries for auto-inferring metadata on new items
    let known_brands = sqlx::query_as::<_, BrandRef>(
        r#"SELECT id, name FROM brands WHERE user_id = $1 OR user_id IS NULL"#
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let known_subcats = sqlx::query_as::<_, SubcategoryRef>(
        r#"SELECT id, name, category_id FROM "clothing-subcategories""#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let known_colors = sqlx::query_as::<_, ColorRef>(
        r#"SELECT id FROM colors"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut imported_count = 0;
    let mut messages_processed = 0;
    let mut new_items_created = 0;
    let mut summary = Vec::new();

    let mut messages_to_process = raw_messages;
    messages_to_process.reverse();

    for msg in messages_to_process {
        if msg.author.bot {
            continue;
        }

        let already_synced = msg
            .reactions
            .iter()
            .any(|r| r.me && r.emoji.name.as_deref() == Some("✅"));
        if already_synced {
            continue;
        }

        // 1. Check for "!link <code>" or "/link <code>" commands
        let trimmed_content = msg.content.trim();
        let words: Vec<&str> = trimmed_content.split_whitespace().collect();
        if words.first().map(|w| w.eq_ignore_ascii_case("!link") || w.eq_ignore_ascii_case("/link")).unwrap_or(false) {
            messages_processed += 1;
            if let Some(code) = words.get(1) {
                let code_clean = code.trim();
                let linked_user = sqlx::query_scalar::<_, Uuid>(
                    r#"
                    UPDATE public."user-settings"
                    SET discord_user_id = $1, discord_link_code = NULL, discord_link_expires_at = NULL, updated_at = now()
                    WHERE discord_link_code = $2 AND discord_link_expires_at > now()
                    RETURNING user_id
                    "#
                )
                .bind(&msg.author.id)
                .bind(code_clean)
                .fetch_optional(pool)
                .await;

                match linked_user {
                    Ok(Some(uid)) => {
                        let _ = discord.add_reaction(&msg.id, "✅").await;
                        let reply = format!("✅ **Successfully connected!** Your Discord account (@{}) is now paired with your Cloth Web wardrobe (`{uid}`). You can now send your daily outfits here!", msg.author.username);
                        let _ = discord.send_reply(&msg.id, &reply).await;
                    }
                    Ok(None) => {
                        let _ = discord.add_reaction(&msg.id, "❌").await;
                        let reply = "❌ **Invalid or expired link code.** Please generate a new 6-digit code in Cloth Web Settings.".to_string();
                        let _ = discord.send_reply(&msg.id, &reply).await;
                    }
                    Err(e) => {
                        eprintln!("Error linking Discord account: {e}");
                    }
                }
            } else {
                let _ = discord.add_reaction(&msg.id, "❓").await;
                let reply = "ℹ️ **Usage:** Send `!link <6-digit-code>` to connect your Cloth Web account.".to_string();
                let _ = discord.send_reply(&msg.id, &reply).await;
            }
            continue;
        }

        // 2. Identify author and verify account linking
        let author_discord_id = &msg.author.id;
        let mut target_user_id: Option<Uuid> = sqlx::query_scalar::<_, Uuid>(
            r#"SELECT user_id FROM public."user-settings" WHERE discord_user_id = $1"#
        )
        .bind(author_discord_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        // Pre-link or default user fallback via environment variable if configured
        let creator_id = std::env::var("DISCORD_CREATOR_ID").unwrap_or_default();
        if target_user_id.is_none() && !creator_id.is_empty() && author_discord_id == &creator_id {
            let _ = sqlx::query(
                r#"
                INSERT INTO public."user-settings" (user_id, discord_user_id, updated_at)
                VALUES ($1, $2, now())
                ON CONFLICT (user_id) DO UPDATE SET discord_user_id = EXCLUDED.discord_user_id, updated_at = now();
                "#
            )
            .bind(user_id)
            .bind(author_discord_id)
            .execute(pool)
            .await;

            target_user_id = Some(user_id);
        }

        let effective_user_id = match target_user_id {
            Some(uid) => uid,
            None => {
                let _ = discord.add_reaction(&msg.id, "⚠️").await;
                let reply = format!(
                    "⚠️ **Account Not Linked:** Your Discord account (@{}) is not linked to a Cloth Web account.\n\nTo connect:\n1. Open Cloth Web -> **Settings** -> **Discord Integration**\n2. Click **Connect Discord** to generate your 6-digit code\n3. Send `!link <code>` here.",
                    msg.author.username
                );
                let _ = discord.send_reply(&msg.id, &reply).await;
                continue;
            }
        };

        // When syncing for a specific user, only ingest their messages
        if effective_user_id != user_id {
            continue;
        }

        let outfits = parse_message_outfits(&msg.content, &msg.timestamp, &msg.id);
        if outfits.is_empty() {
            continue;
        }

        messages_processed += 1;
        let mut all_outfits_succeeded = true;
        let mut msg_confirmation_lines = Vec::new();

        for parsed in outfits {
            let mut matched_item_ids: Vec<Uuid> = Vec::new();
            let mut matched_names: Vec<String> = Vec::new();
            let mut unmatched_queries: Vec<String> = Vec::new();
            let mut pending_new_items: Vec<PendingNewItem> = Vec::new();

            for query in &parsed.item_queries {
                let (is_new, clean_query) = extract_new_item_marker(query);

                if is_new {
                    let display_name = to_title_case(&clean_query);
                    let norm_q = clean_query.to_lowercase();

                    // Infer Brand (e.g. "Red Wing" in "Red Wing Boots")
                    let mut detected_brand_id: Option<Uuid> = None;
                    let mut detected_brand_name = String::new();
                    for b in &known_brands {
                        let b_lower = b.name.to_lowercase();
                        if !b_lower.is_empty() && norm_q.contains(&b_lower) {
                            detected_brand_id = Some(b.id);
                            detected_brand_name = b.name.clone();
                            break;
                        }
                    }

                    // Infer Subcategory & Category (e.g. "boots", "jeans", "shirt")
                    let mut detected_subcat_id: Option<i32> = None;
                    let mut detected_cat_id: Option<i32> = None;
                    for sc in &known_subcats {
                        let sc_lower = sc.name.to_lowercase();
                        if !sc_lower.is_empty() && norm_q.contains(&sc_lower) {
                            detected_subcat_id = Some(sc.id);
                            detected_cat_id = Some(sc.category_id);
                            break;
                        }
                    }

                    // Infer Color (e.g. "white", "black", "navy")
                    let mut detected_color_id: Option<String> = None;
                    let q_tokens: Vec<&str> = norm_q.split_whitespace().collect();
                    for c in &known_colors {
                        if q_tokens.contains(&c.id.as_str()) {
                            detected_color_id = Some(c.id.clone());
                            break;
                        }
                    }

                    let new_id = Uuid::new_v4();
                    pending_new_items.push(PendingNewItem {
                        id: new_id,
                        name: display_name.clone(),
                        brand_id: detected_brand_id,
                        brand_name: detected_brand_name,
                        category_id: detected_cat_id,
                        subcategory_id: detected_subcat_id,
                        color_id: detected_color_id,
                    });

                    matched_item_ids.push(new_id);
                    matched_names.push(format!("✨ **{display_name}** *(new item)*"));
                } else {
                    // Match against existing wardrobe candidates
                    let mut best_candidate: Option<(&ClothingCandidate, f64)> = None;

                    for cand in &candidates {
                        let score = score_item_match(query, cand);
                        if score >= 0.55 {
                            if let Some((_, best_score)) = best_candidate {
                                if score > best_score {
                                    best_candidate = Some((cand, score));
                                }
                            } else {
                                best_candidate = Some((cand, score));
                            }
                        }
                    }

                    if let Some((cand, _score)) = best_candidate {
                        matched_item_ids.push(cand.id);
                        matched_names.push(cand.display_name());
                    } else {
                        unmatched_queries.push(query.clone());
                    }
                }
            }

            if matched_item_ids.is_empty() {
                all_outfits_succeeded = false;
                continue;
            }

            let mut notes = String::new();
            if !unmatched_queries.is_empty() {
                notes = format!("(Unmatched items: {})", unmatched_queries.join(", "));
            }

            let mut tx = pool
                .begin()
                .await
                .map_err(|e| format!("Transaction start failed: {e}"))?;

            // 1. Insert any newly created clothing items
            for new_item in &pending_new_items {
                if let Err(e) = sqlx::query(
                    r#"
                    INSERT INTO "clothing-items" 
                    (id, user_id, name, brand_id, category_id, subcategory_id, color_id, owned_since, is_active, wear_count, created_at, updated_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8::date, true, 0, now(), now())
                    "#,
                )
                .bind(new_item.id)
                .bind(user_id)
                .bind(&new_item.name)
                .bind(new_item.brand_id)
                .bind(new_item.category_id)
                .bind(new_item.subcategory_id)
                .bind(&new_item.color_id)
                .bind(&parsed.date)
                .execute(&mut *tx)
                .await
                {
                    eprintln!("Failed to create new clothing item {}: {e}", new_item.name);
                } else {
                    new_items_created += 1;
                    candidates.push(ClothingCandidate {
                        id: new_item.id,
                        name: new_item.name.clone(),
                        brand: new_item.brand_name.clone(),
                        category: String::new(),
                        subcategory: String::new(),
                        color: new_item.color_id.clone().unwrap_or_default(),
                    });
                }
            }

            // 2. Insert the outfit
            let outfit_row: Result<(Uuid,), _> = sqlx::query_as(
                r#"
                INSERT INTO outfits (user_id, date, notes, is_active, created_at, updated_at)
                VALUES ($1, $2::timestamptz, $3, true, now(), now())
                RETURNING id
                "#,
            )
            .bind(user_id)
            .bind(&parsed.date)
            .bind(if notes.is_empty() { None } else { Some(&notes) })
            .fetch_one(&mut *tx)
            .await;

            let outfit_id = match outfit_row {
                Ok((id,)) => id,
                Err(e) => {
                    eprintln!("Failed to insert outfit for {}: {e}", parsed.date);
                    all_outfits_succeeded = false;
                    continue;
                }
            };

            // 3. Link items to the outfit
            for item_id in &matched_item_ids {
                if let Err(e) = sqlx::query(
                    r#"
                    INSERT INTO "outfit-items" (outfit_id, clothing_item_id)
                    VALUES ($1, $2)
                    "#,
                )
                .bind(outfit_id)
                .bind(item_id)
                .execute(&mut *tx)
                .await
                {
                    eprintln!("Failed to link outfit item: {e}");
                }
            }

            // 4. Increment wear counts for all linked items
            if let Err(e) = sqlx::query(
                r#"
                UPDATE "clothing-items" ci
                SET wear_count = COALESCE(ci.wear_count, 0) + 1, updated_at = now()
                FROM (SELECT DISTINCT clothing_item_id FROM "outfit-items" WHERE outfit_id = $1) s
                WHERE ci.id = s.clothing_item_id AND ci.user_id = $2
                "#,
            )
            .bind(outfit_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await
            {
                eprintln!("Failed to update wear counts: {e}");
            }

            if let Err(e) = tx.commit().await {
                eprintln!("Failed to commit outfit transaction: {e}");
                all_outfits_succeeded = false;
                continue;
            }

            imported_count += 1;
            let summary_line = format!(
                "{} -> [{}]",
                parsed.date,
                matched_names.join(", ")
            );
            summary.push(summary_line.clone());

            let mut confirmation = format!("📅 **{}**:\n", parsed.date);
            for name in &matched_names {
                confirmation.push_str(&format!(" • {name}\n"));
            }
            if !unmatched_queries.is_empty() {
                confirmation.push_str(&format!(
                    " ⚠️ *Unmatched:* {}\n",
                    unmatched_queries.join(", ")
                ));
            }
            msg_confirmation_lines.push(confirmation);
        }

        if all_outfits_succeeded && !msg_confirmation_lines.is_empty() {
            let _ = discord.add_reaction(&msg.id, "✅").await;
            let reply_text = format!(
                "✅ **Logged outfit successfully:**\n\n{}",
                msg_confirmation_lines.join("\n")
            );
            let _ = discord.send_reply(&msg.id, &reply_text).await;
        } else if !msg_confirmation_lines.is_empty() {
            let _ = discord.add_reaction(&msg.id, "⚠️").await;
            let reply_text = format!(
                "⚠️ **Partially logged:**\n\n{}",
                msg_confirmation_lines.join("\n")
            );
            let _ = discord.send_reply(&msg.id, &reply_text).await;
        }
    }

    Ok(SyncResult {
        imported_count,
        messages_processed,
        new_items_created,
        summary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_new_item_marker() {
        assert_eq!(
            extract_new_item_marker("red wing boots (new)"),
            (true, "red wing boots".into())
        );
        assert_eq!(
            extract_new_item_marker("(new) red wing boots"),
            (true, "red wing boots".into())
        );
        assert_eq!(
            extract_new_item_marker("red wing boots (NEW)"),
            (true, "red wing boots".into())
        );
        assert_eq!(
            extract_new_item_marker("red wing boots [new]"),
            (true, "red wing boots".into())
        );
        assert_eq!(
            extract_new_item_marker("benzak white tee"),
            (false, "benzak white tee".into())
        );
    }

    #[test]
    fn test_to_title_case() {
        assert_eq!(to_title_case("red wing boots"), "Red Wing Boots");
        assert_eq!(to_title_case("xx002 jeans"), "Xx002 Jeans");
    }

    #[test]
    fn test_parse_date_line() {
        assert_eq!(parse_date_line("26/10", 2026), Some("2026-10-26".into()));
        assert_eq!(parse_date_line("03/07", 2026), Some("2026-07-03".into()));
        assert_eq!(parse_date_line("26/10/2026", 2025), Some("2026-10-26".into()));
        assert_eq!(parse_date_line("26-10-2026", 2025), Some("2026-10-26".into()));
        assert_eq!(parse_date_line("benzak white tee", 2026), None);
    }

    #[test]
    fn test_parse_message_outfits_single() {
        let text = "26/10\nbenzak white tee\nxx002 jeans\nred wing boots (new)";
        let parsed = parse_message_outfits(text, "2026-10-26T16:00:00Z", "msg-1");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].date, "2026-10-26");
        assert_eq!(
            parsed[0].item_queries,
            vec!["benzak white tee", "xx002 jeans", "red wing boots (new)"]
        );
    }

    #[test]
    fn test_score_item_match() {
        let cand = ClothingCandidate {
            id: Uuid::new_v4(),
            name: "White Tee".into(),
            brand: "Benzak".into(),
            category: "base-layer".into(),
            subcategory: "t-shirt".into(),
            color: "white".into(),
        };

        let score = score_item_match("benzak white tee", &cand);
        assert!(score >= 0.80, "Expected high score, got {score}");
    }
}
