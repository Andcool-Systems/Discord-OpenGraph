// root.rs
#![allow(clippy::cargo)]
#![deny(
    deprecated,
    dead_code,
    unused,
    unreachable_code,
    large_assignments,
    unstable_features,
    unused_imports,
    unused_mut,
    unused_variables,
    warnings,
    unsafe_code,
    clippy::all,
    clippy::pedantic,
    clippy::expect_used,
    clippy::float_cmp,
    clippy::panic,
    clippy::shadow_unrelated,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::indexing_slicing,
    clippy::unwrap_in_result,
    clippy::verbose_file_reads,
    clippy::module_name_repetitions,
    clippy::similar_names,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::inefficient_to_string,
    clippy::large_enum_variant,
    clippy::manual_assert,
    clippy::map_entry,
    clippy::missing_enforced_import_renames,
    clippy::modulo_arithmetic,
    clippy::needless_pass_by_value,
    clippy::new_without_default,
    clippy::no_effect,
    clippy::panic_in_result_fn,
    clippy::range_plus_one,
    clippy::redundant_allocation,
    clippy::redundant_clone,
    clippy::redundant_field_names,
    clippy::redundant_pub_crate,
    clippy::single_match_else,
    clippy::string_add,
    clippy::string_to_string,
    clippy::too_many_arguments,
    clippy::trivial_regex,
    clippy::type_complexity,
    clippy::unimplemented,
    clippy::unnecessary_cast,
    clippy::unnecessary_wraps,
    clippy::unneeded_field_pattern,
    clippy::unreachable,
    clippy::unused_self,
    clippy::useless_conversion,
    clippy::wildcard_enum_match_arm,
    clippy::write_with_newline,
    clippy::wrong_self_convention
)]

use {
    actix_web::{
        http::header::HeaderValue,
        middleware::Logger,
        web,
        App,
        HttpRequest,
        HttpResponse,
        HttpServer,
        Result
    },
    chrono::{DateTime, Utc},
    colored::{ColoredString, Colorize as _},
    env_logger::{
        fmt::{Formatter, Timestamp},
        Builder
    },
    log::{error, Level, LevelFilter, Record},
    reqwest::Client,
    serde::{Deserialize, Serialize},
    serde_json::Value,
    sqlx::{
        sqlite::{SqlitePool, SqliteRow},
        Pool,
        Row,
        Sqlite
    },
    std::{env, error::Error, fs, io::Write as _, path::Path, process::exit},
    thiserror::Error
};

#[derive(Debug, Error)]
enum AppError
{
    #[error("Request Error: {0}")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Database Error: {0}")]
    DatabaseError(#[from] sqlx::Error),

    #[error("Missing Token")]
    MissingToken,

    #[error("Not Found: {0}")]
    NotFound(String),

    #[error("Ser/De error: {0}")]
    SerDe(#[from] serde_json::Error)
}

impl actix_web::error::ResponseError for AppError
{
    fn error_response(&self) -> HttpResponse
    {
        let (status_code, message) = match self {
            AppError::ReqwestError(err) => {
                (
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Internal Server Error, {err}")
                )
            },
            AppError::DatabaseError(_) => {
                (
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Database Error".to_string()
                )
            },
            AppError::MissingToken => {
                (
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    "Missing environment variable".to_string()
                )
            },
            AppError::NotFound(ref msg) => {
                (
                    actix_web::http::StatusCode::NOT_FOUND,
                    msg.as_str().to_string()
                )
            },
            AppError::SerDe(err) => {
                (
                    actix_web::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Ser/De error: {err}")
                )
            },
        };

        HttpResponse::build(status_code)
            .content_type("application/json")
            .body(
                serde_json::json!({
                    "status": "error",
                    "message": message,
                })
                .to_string()
            )
    }
}

#[derive(Serialize, Deserialize)]
struct UserData
{
    global_name: Option<String>,
    username: String,
    banner_color: Option<String>,
    avatar: String,
    bio: String
}

#[derive(Serialize, Deserialize)]
struct ApiResponse
{
    status: String,
    data: Option<UserData>,
    message: Option<String>
}

async fn get_user_data(client: &Client, uid: &str) -> Result<UserData, AppError>
{
    let token: String = env::var("TOKEN").map_err(|_| AppError::MissingToken)?;

    let profile_url: String = format!("https://discord.com/api/v10/users/{uid}/profile");
    let profile_response: String = client
        .get(&profile_url)
        .header("Authorization", token.clone())
        .send()
        .await?
        .text()
        .await?;

    let profile_data: Value = serde_json::from_str(&profile_response)?;
    let bio: String = profile_data
        .get("user")
        .and_then(|user: &Value| user.get("bio"))
        .and_then(|bio: &Value| bio.as_str())
        .unwrap_or("")
        .to_string();

    let user_url: String = format!("https://discord.com/api/v10/users/{uid}");
    let user_response: String = client
        .get(&user_url)
        .header("Authorization", token)
        .send()
        .await?
        .text()
        .await?;

    let user_data_json: Value = serde_json::from_str(&user_response)?;

    let user_data = UserData {
        global_name: user_data_json
            .get("global_name")
            .and_then(|v: &Value| v.as_str())
            .map(std::string::ToString::to_string),
        username: user_data_json
            .get("username")
            .and_then(|v: &Value| v.as_str())
            .unwrap_or("")
            .to_string(),
        banner_color: user_data_json
            .get("banner_color")
            .and_then(|v: &Value| v.as_str())
            .map(std::string::ToString::to_string),
        avatar: user_data_json
            .get("avatar")
            .and_then(|v: &Value| v.as_str())
            .unwrap_or("")
            .to_string(),
        bio
    };

    Ok(user_data)
}

async fn fetch_from_cache(pool: &SqlitePool, uid: &str) -> Result<Option<UserData>, AppError>
{
    let six_hours_ago: DateTime<Utc> = Utc::now() - chrono::Duration::hours(6);

    let cached_row: Option<SqliteRow> =
        sqlx::query("SELECT * FROM user_cache WHERE uid = ? AND timestamp > ?")
            .bind(uid)
            .bind(six_hours_ago.naive_utc())
            .fetch_optional(pool)
            .await?;

    Ok(cached_row.map(|row: SqliteRow| {
        UserData {
            global_name: row.get("global_name"),
            username: row.get("username"),
            banner_color: row.get("banner_color"),
            avatar: row.get("avatar"),
            bio: row.get("bio")
        }
    }))
}

async fn save_to_cache(pool: &SqlitePool, uid: &str, data: &UserData) -> Result<(), AppError>
{
    sqlx::query(
        "INSERT OR REPLACE INTO user_cache (uid, global_name, username, banner_color, avatar, \
         bio, timestamp) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
    .bind(uid)
    .bind(&data.global_name)
    .bind(&data.username)
    .bind(&data.banner_color)
    .bind(&data.avatar)
    .bind(&data.bio)
    .bind(Utc::now().naive_utc())
    .execute(pool)
    .await?;

    Ok(())
}

async fn root() -> HttpResponse
{
    HttpResponse::Found()
        .append_header((
            "Location",
            "https://github.com/Andcool-Systems/Discord-OpenGraph/tree/main"
        ))
        .finish()
}

#[derive(Clone, Serialize, Deserialize)]
struct UserID
{
    id: String
}

async fn uid(
    data: web::Query<UserID>,
    req: HttpRequest,
    pool: web::Data<SqlitePool>
) -> Result<HttpResponse, AppError>
{
    let uid: String = data.id.clone();

    if uid == "favicon.ico" {
        return Ok(HttpResponse::NotFound().finish());
    }

    let client: Client = Client::new();
    let mut user_data: Option<UserData> = fetch_from_cache(&pool, &uid).await?;

    if user_data.is_none() {
        let fetched_data: UserData = get_user_data(&client, &uid).await?;
        user_data = Some(fetched_data);
        let user_data_ref: &UserData = user_data
            .as_ref()
            .ok_or(AppError::NotFound("User not found".into()))?;
        save_to_cache(&pool, &uid, user_data_ref).await?;
    }

    let user_data: UserData = user_data.ok_or(AppError::NotFound("User not found".into()))?;

    if req
        .headers()
        .get("accept")
        .map_or(false, |v: &HeaderValue| {
            v.to_str().unwrap_or("") == "application/json"
        })
    {
        let response_data = ApiResponse {
            status: "success".into(),
            data: Some(user_data),
            message: None
        };
        Ok(HttpResponse::Ok().json(response_data))
    }
    else {
        let avatar_url = format!(
            "https://cdn.discordapp.com/avatars/{}/{}?size=2048",
            uid, user_data.avatar
        );
        let bio: String = user_data.bio;
        let html_content: String = format!(
            r#"
            <!DOCTYPE html>
            <html>
            <head>
                <meta property="og:title" content="{}">
                <meta name="theme-color" content="{}">
                <meta property="og:url" content="https://discord.com/users/{uid}" />
                <meta property="og:site_name" content="Discord" />
                <meta property="og:image" content="{avatar_url}" />
                <meta property="og:description" content="{bio}" />
            </head>
            <body>
            </body>
            <script>
                window.location.replace("https://discord.com/users/{uid}");
            </script>
            </html>
            "#,
            user_data
                .global_name
                .as_deref()
                .unwrap_or(&user_data.username),
            user_data.banner_color.as_deref().unwrap_or("#2563eb"),
        );
        Ok(HttpResponse::Ok().body(html_content))
    }
}

async fn initialize_database() -> Result<Pool<Sqlite>, Box<dyn Error>>
{
    let database_path: &String =
        &(env::var("SQLITE_DB_PATH").unwrap_or_else(|_| "cache.db".to_string()));

    if !Path::new(database_path).exists() {
        fs::File::create(database_path)?;
    }

    let database_url: String = format!("sqlite:{database_path}");
    let pool: Pool<Sqlite> = SqlitePool::connect(&database_url).await?;

    sqlx::query(
        "
        CREATE TABLE IF NOT EXISTS user_cache (
            uid TEXT PRIMARY KEY,
            global_name TEXT,
            username TEXT NOT NULL,
            banner_color TEXT,
            avatar TEXT NOT NULL,
            bio TEXT,
            timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
        );
        "
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

#[actix_web::main]
async fn main() -> std::io::Result<()>
{
    Builder::from_default_env()
        .format(|buf: &mut Formatter, record: &Record| {
            let ts: Timestamp = buf.timestamp();

            let level_color: ColoredString = match record.level() {
                Level::Error => "ERROR".red().bold(),
                Level::Warn => "WARN".yellow().bold(),
                Level::Info => "INFO".blue().bold(),
                Level::Debug => "DEBUG".green().bold(),
                Level::Trace => "TRACE".purple().bold()
            };

            writeln!(
                buf,
                "{ts} [{level_color}] - {}",
                record.args().to_string().bold().truecolor(159, 146, 104)
            )
        })
        .filter_level(LevelFilter::Info)
        .init();

    dotenv::dotenv().ok();

    if env::var("TOKEN").is_err() {
        error!("Missing TOKEN environment variable");
        exit(1);
    }

    let pool: Pool<Sqlite> = initialize_database()
        .await
        .unwrap_or_else(|e: Box<dyn Error>| {
            error!("Error initializing database: {e:?}");
            exit(2)
        });

    let bind_addr: String = {
        let bind_ip: String = env::var("BIND_IP").unwrap_or("0.0.0.0".to_string());
        let bind_port: String = env::var("BIND_PORT").unwrap_or("6969".to_string());

        format!("{bind_ip}:{bind_port}")
    };

    HttpServer::new(move || {
        App::new()
            .app_data(web::Data::new(pool.clone()))
            .wrap(Logger::new(
                "%a %r status=%s size_in_bytes=%b serve_time=%Ts"
            ))
            .route("/", web::get().to(root))
            .route("/info", web::get().to(uid))
    })
    .bind(bind_addr)?
    .run()
    .await
}
