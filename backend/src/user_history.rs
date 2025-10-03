use actix_web::{get, web, HttpRequest, HttpResponse};
use chrono::DateTime;
use serde::{Deserialize, Serialize};
use sha2::Digest;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use std::collections::{HashMap, HashSet};

use crate::{
    errors::ServiceError, get_ip_address,
    middleware,
    models::{Comment, Post},
    rate_limiter,
};

/// APIクエリパラメータ
#[derive(Deserialize, Debug)]
pub struct HistoryQuery {
    pub user_part: Option<String>,
    pub ip_part: Option<String>,
    pub device_part: Option<String>,
    pub logic: Option<String>, // "and" or "or"
    pub sort: Option<String>,  // "time_desc", "time_asc", "thread_desc", "thread_asc"
}

/// レスポンスでスレッドとコメントを統一的に扱うためのenum
#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
pub enum HistoryItem {
    Post(Post),
    Comment(Comment),
}

/// 検索結果のサマリー情報
#[derive(Serialize, Debug, Default)]
pub struct HistorySummary {
    pub first_seen: Option<DateTime<chrono::Utc>>,
    pub last_seen: Option<DateTime<chrono::Utc>>,
    pub created_thread_count: i64,
    pub comment_count: i64,
    pub total_contribution_count: i64,
    /// (スレッドタイトル, そのスレッド内での総投稿数)
    pub created_threads: Vec<(String, i64)>,
    /// (スレッドタイトル, そのスレッド内での総投稿数)
    pub commented_in_threads: Vec<(String, i64)>,
}

/// APIレスポンス全体
#[derive(Serialize, Debug)]
pub struct HistoryResponse {
    pub summary: HistorySummary,
    pub items: Vec<HistoryItem>,
}

/// 指定されたIDの各部分文字列に一致する投稿履歴を取得します。
#[get("/by-id-parts")]
pub async fn get_history_by_id_parts(
    pool: web::Data<PgPool>,
    req: HttpRequest,
    query: web::Query<HistoryQuery>,
    user: web::ReqData<middleware::AuthenticatedUser>,
) -> Result<HttpResponse, ServiceError> {
    // --- 0. レート制限チェック ---
    let user_id = user.user_id;
    let ip_address = get_ip_address(&req);
    let device_info = req
        .headers()
        .get("User-Agent")
        .and_then(|ua| ua.to_str().ok())
        .unwrap_or("unknown");

    // 永続ハッシュを生成してレート制限に渡す
    // (この部分は identity.rs からの借用ですが、レート制限のためだけに必要)
    let ip_hash = {
        let mut hasher = sha2::Sha256::new();
        hasher.update(ip_address.0.as_bytes()); // タプルの最初の要素 (切り詰められたIP) を使用
        hex::encode(hasher.finalize())
    };
    let device_hash = {
        let mut hasher = sha2::Sha256::new();
        hasher.update(device_info.as_bytes());
        hex::encode(hasher.finalize())
    };

    let mut tx = pool.begin().await?;
    rate_limiter::check_and_track_rate_limits(
        &mut tx,
        user_id,
        &ip_hash,
        &device_hash,
        crate::models::RateLimitActionType::SearchHistory,
    )
    .await?;
    tx.commit().await?;

    // --- 1. 動的なクエリの構築 ---
    let mut posts_query: QueryBuilder<Postgres> = QueryBuilder::new(
        r#"SELECT p.id, p.title, p.body, p.author_name, p.created_at, p.updated_at, p.board_id, p.deleted_at, p.user_id,
                  p.archived_at, p.last_activity_at, p.display_user_id, p.permanent_user_hash, p.permanent_ip_hash,
                  p.permanent_device_hash, p.level_at_creation, u.level
           FROM posts p
           LEFT JOIN users u ON p.user_id = u.id
           WHERE p.deleted_at IS NULL AND ("#,
    );
    let mut comments_query: QueryBuilder<Postgres> = QueryBuilder::new(
        // JOINを追加してスレッドタイトルを取得
        r#"SELECT c.id, c.body, c.post_id, c.user_id, c.author_name, c.created_at, c.updated_at, c.display_user_id,
                   c.permanent_user_hash, c.permanent_ip_hash, c.permanent_device_hash,
                   c.level_at_creation, u.level, p.title as post_title
            FROM comments c
            JOIN posts p ON c.post_id = p.id
            LEFT JOIN users u ON c.user_id = u.id
            WHERE p.deleted_at IS NULL AND ("#,
    );

    let logic_separator = if query.logic.as_deref() == Some("or") {
        " OR "
    } else {
        " AND "
    };
    let mut condition_count = 0;

    // 各ID部分の条件を追加
    for (part, column_name) in [
        (query.user_part.as_ref(), "display_id_user"),
        (query.ip_part.as_ref(), "display_id_ip"),
        (query.device_part.as_ref(), "display_id_device"),
    ] {
        if let Some(p) = part.filter(|s| !s.is_empty()) {
            if condition_count > 0 {
                posts_query.push(logic_separator);
                comments_query.push(logic_separator);
            }
            // IDの長さが変更されたため、完全一致(=)から前方一致(LIKE)に変更します。
            // これにより、新旧両方のフォーマット（4文字と8文字）を検索できます。
            // ユーザーが入力した文字列で始まるIDを検索するために、末尾に'%'を追加します。
            let pattern = format!("{}%", p);
            posts_query
                .push("p.")
                .push(column_name)
                .push(" LIKE ")
                .push_bind(pattern.clone());
            comments_query
                .push("c.")
                .push(column_name)
                .push(" LIKE ")
                .push_bind(pattern);
            condition_count += 1;
        }
    }

    if condition_count == 0 {
        return Err(ServiceError::BadRequest(
            "少なくとも1つのID部分を指定してください。".to_string(),
        ));
    }

    posts_query.push(")");
    comments_query.push(")");

    // --- 2. データベースから投稿とコメントを並行して検索 ---
    let posts_task = posts_query
        .build_query_as::<Post>()
        .fetch_all(pool.get_ref());
    let comments_task = comments_query
        .build_query_as::<Comment>()
        .fetch_all(pool.get_ref());

    let (posts_result, comments_result) = tokio::join!(posts_task, comments_task);
    let posts = posts_result?;
    let mut comments = comments_result?;

    if posts.is_empty() && comments.is_empty() {
        return Err(ServiceError::NotFound(
            "指定されたIDを持つ投稿は見つかりませんでした。".to_string(),
        ));
    }

    // --- START: 正しいレスナンバーを計算して付与 ---
    if !comments.is_empty() {
        // 1. 関連するスレッドIDをすべて集める
        let relevant_thread_ids: Vec<i32> = comments
            .iter()
            .map(|c| c.post_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        // 2. 関連スレッドの全コメントを投稿順に取得し、レスナンバーを計算するためのマップを作成
        let all_thread_comments: Vec<(i32, i32)> = sqlx::query_as(
            "SELECT post_id, id FROM comments WHERE post_id = ANY($1) ORDER BY post_id, created_at ASC"
        )
        .bind(&relevant_thread_ids)
        .fetch_all(pool.get_ref())
        .await?;

        let mut response_number_map: HashMap<i32, i64> = HashMap::new();
        if !all_thread_comments.is_empty() {
            let mut current_thread_id = all_thread_comments[0].0;
            let mut counter = 2; // 1はスレ本体なので2から開始
            for (post_id, comment_id) in all_thread_comments {
                if post_id != current_thread_id {
                    current_thread_id = post_id;
                    counter = 2; // スレッドが変わったらカウンターをリセット
                }
                response_number_map.insert(comment_id, counter);
                counter += 1;
            }
        }

        // 3. 検索結果のコメントにレス番号をセット
        for comment in &mut comments {
            comment.response_number = response_number_map.get(&comment.id).cloned();
        }
    }
    // --- END: 正しいレスナンバーを計算して付与 ---

    // --- 3. 結果のマージ、サマリー計算、ソート ---
    let mut items: Vec<HistoryItem> = posts
        .iter()
        .cloned()
        .map(HistoryItem::Post)
        .chain(comments.iter().cloned().map(HistoryItem::Comment))
        .collect();

    let summary = calculate_summary(&posts, &comments, pool.get_ref()).await?;

    // --- START: ソート機能の修正 ---
    // スレッドごとのソートを正しく機能させるため、検索結果に含まれるすべてのスレッドの作成日時を収集します。

    // 1. 検索結果のスレッドから作成日時をマップに格納
    let mut thread_creation_times: HashMap<i32, DateTime<chrono::Utc>> =
        posts.iter().map(|p| (p.id, p.created_at)).collect();

    // 2. コメントが属するスレッドのうち、まだマップにないもののIDをリストアップ
    let post_ids_from_comments: HashSet<i32> = comments.iter().map(|c| c.post_id).collect();
    let missing_post_ids: Vec<i32> = post_ids_from_comments
        .into_iter()
        .filter(|id| !thread_creation_times.contains_key(id))
        .collect();

    // 3. 不足しているスレッドの作成日時をDBから取得してマップに追加
    if !missing_post_ids.is_empty() {
        let missing_times: Vec<(i32, DateTime<chrono::Utc>)> =
            sqlx::query_as("SELECT id, created_at FROM posts WHERE id = ANY($1)")
                .bind(&missing_post_ids)
                .fetch_all(pool.get_ref())
                .await?;

        thread_creation_times.extend(missing_times);
    }

    // 4. 完成したマップを使ってソートを実行
    sort_items(
        &mut items,
        query.sort.as_deref().unwrap_or("thread_desc"),
        &thread_creation_times,
    );
    // --- END: ソート機能の修正 ---

    // レスポンスから機密情報を除去
    let sanitized_items = sanitize_items(items);

    Ok(HttpResponse::Ok().json(HistoryResponse {
        summary,
        items: sanitized_items,
    }))
}

/// 検索結果からサマリー情報を計算するヘルパー関数
async fn calculate_summary(
    posts: &[Post],
    comments: &[Comment],
    pool: &PgPool,
) -> Result<HistorySummary, ServiceError> {
    let all_times: Vec<_> = posts
        .iter()
        .map(|p| p.created_at)
        .chain(comments.iter().map(|c| c.created_at))
        .collect();

    // スレッドごとの投稿数を集計
    let mut contribution_counts: HashMap<i32, i64> = HashMap::new();
    for p in posts {
        *contribution_counts.entry(p.id).or_default() += 1;
    }
    for c in comments {
        *contribution_counts.entry(c.post_id).or_default() += 1;
    }

    // 作成したスレッドの一覧を作成
    let created_threads: Vec<(String, i64)> = posts
        .iter()
        .map(|p| {
            (
                p.title.clone(),
                *contribution_counts.get(&p.id).unwrap_or(&0),
            )
        })
        .collect();

    // コメントしたスレッドの一覧を作成 (自分が作成したスレッドは除く)
    let created_thread_ids: HashSet<i32> = posts.iter().map(|p| p.id).collect();
    let commented_thread_ids: Vec<i32> = comments
        .iter()
        .map(|c| c.post_id)
        .filter(|id| !created_thread_ids.contains(id))
        .collect::<HashSet<_>>() // 重複を除去
        .into_iter()
        .collect();

    let mut commented_in_threads = vec![];
    if !commented_thread_ids.is_empty() {
        let post_titles: HashMap<i32, String> =
            sqlx::query("SELECT id, title FROM posts WHERE id = ANY($1)")
                .bind(&commented_thread_ids)
                .fetch_all(pool)
                .await?
                .into_iter()
                .map(|row| (row.get("id"), row.get("title")))
                .collect();

        commented_in_threads = commented_thread_ids
            .iter()
            .filter_map(|id| {
                post_titles
                    .get(id)
                    .map(|title| (title.clone(), *contribution_counts.get(id).unwrap_or(&0)))
            })
            .collect();
    }

    Ok(HistorySummary {
        first_seen: all_times.iter().min().cloned(),
        last_seen: all_times.iter().max().cloned(),
        created_thread_count: posts.len() as i64,
        comment_count: comments.len() as i64,
        total_contribution_count: (posts.len() + comments.len()) as i64,
        created_threads,
        commented_in_threads,
    })
}

/// 投稿アイテムをソートするヘルパー関数
fn sort_items(
    items: &mut [HistoryItem],
    sort_order: &str,
    thread_creation_times: &HashMap<i32, DateTime<chrono::Utc>>,
) {
    match sort_order {
        "time_asc" => items.sort_by_key(get_item_time),
        "time_desc" => items.sort_by_key(|item| std::cmp::Reverse(get_item_time(item))),
        "thread_asc" | "thread_desc" => {
            items.sort_by(|a, b| {
                let (thread_time_a, item_time_a) =
                    get_thread_and_item_time(a, thread_creation_times);
                let (thread_time_b, item_time_b) =
                    get_thread_and_item_time(b, thread_creation_times);

                let thread_order = if sort_order == "thread_asc" {
                    thread_time_a.cmp(&thread_time_b)
                } else {
                    thread_time_b.cmp(&thread_time_a)
                };

                thread_order.then_with(|| item_time_a.cmp(&item_time_b))
            });
        }
        // デフォルトのソート順（スレッドの新しい順）。予期しない値が渡された場合のフォールバック。
        _ => {
            items.sort_by(|a, b| {
                let (thread_time_a, item_time_a) =
                    get_thread_and_item_time(a, thread_creation_times);
                let (thread_time_b, item_time_b) =
                    get_thread_and_item_time(b, thread_creation_times);

                thread_time_b
                    .cmp(&thread_time_a)
                    .then_with(|| item_time_a.cmp(&item_time_b))
            });
        }
    }
}

fn get_item_time(item: &HistoryItem) -> DateTime<chrono::Utc> {
    match item {
        HistoryItem::Post(p) => p.created_at,
        HistoryItem::Comment(c) => c.created_at,
    }
}

fn get_thread_and_item_time(
    item: &HistoryItem,
    thread_times: &HashMap<i32, DateTime<chrono::Utc>>,
) -> (DateTime<chrono::Utc>, DateTime<chrono::Utc>) {
    match item {
        HistoryItem::Post(p) => (p.created_at, p.created_at),
        HistoryItem::Comment(c) => {
            let thread_time = thread_times
                .get(&c.post_id)
                .cloned()
                .unwrap_or(c.created_at);
            (thread_time, c.created_at)
        }
    }
}

/// レスポンスから機密情報を除去するヘルパー関数
fn sanitize_items(items: Vec<HistoryItem>) -> Vec<HistoryItem> {
    items
        .into_iter()
        .map(|item| match item {
            HistoryItem::Post(mut p) => {
                p.permanent_user_hash = None;
                p.permanent_ip_hash = None;
                p.permanent_device_hash = None;
                HistoryItem::Post(p)
            }
            HistoryItem::Comment(mut c) => {
                c.permanent_user_hash = None;
                c.permanent_ip_hash = None;
                c.permanent_device_hash = None;
                HistoryItem::Comment(c)
            }
        })
        .collect()
}
