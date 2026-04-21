mod cmd;
mod output;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "slab",
    about = "CLI for Slab knowledge base — read, search, sync, and push documentation",
    version
)]
struct Cli {
    /// Slab team subdomain (e.g. "acme" for acme.slab.com)
    #[arg(long, env = "SLAB_TEAM", global = true)]
    team: Option<String>,

    /// Slab API token
    #[arg(long, env = "SLAB_API_TOKEN", global = true, hide_env_values = true)]
    token: Option<String>,

    /// Path to local vault directory
    #[arg(long, env = "SLAB_VAULT", global = true)]
    vault: Option<std::path::PathBuf>,

    /// Output format
    #[arg(long, global = true, default_value = "text")]
    format: output::Format,

    /// Output as JSON (shorthand for --format=json)
    #[arg(long, global = true)]
    json: bool,

    /// Copy output to clipboard
    #[arg(long, global = true)]
    copy: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Authentication commands
    Auth {
        #[command(subcommand)]
        action: cmd::auth::AuthAction,
    },
    /// Vault initialization and info
    Vault {
        #[command(subcommand)]
        action: cmd::vault_cmd::VaultAction,
    },
    /// Get a post by ID, URL, or slug
    #[command(name = "post:get")]
    PostGet {
        /// Post ID or URL
        id: String,
        /// Show raw Quill Delta JSON instead of markdown
        #[arg(long)]
        raw: bool,
    },
    /// List posts
    #[command(name = "post:list")]
    PostList {
        /// Filter by topic ID
        #[arg(long)]
        topic: Option<String>,
    },
    /// Create a new post
    #[command(name = "post:create")]
    PostCreate {
        /// Post title
        title: String,
        /// Topic ID to add the post to
        #[arg(long)]
        topic: Option<String>,
        /// Read content from file (use - for stdin)
        #[arg(long)]
        content_file: Option<String>,
    },
    /// Update a post
    #[command(name = "post:update")]
    PostUpdate {
        /// Post ID or local file path
        id: String,
        /// Read content from file (use - for stdin)
        #[arg(long)]
        content_file: Option<String>,
        /// New title
        #[arg(long)]
        title: Option<String>,
    },
    /// List topics
    #[command(name = "topic:list")]
    TopicList,
    /// Get topic details
    #[command(name = "topic:get")]
    TopicGet {
        /// Topic ID
        id: String,
    },
    /// List posts in a topic
    #[command(name = "topic:posts")]
    TopicPosts {
        /// Topic ID
        id: String,
    },
    /// Search posts
    Search {
        /// Search query
        query: String,
        /// Topic filter
        #[arg(long)]
        topic: Option<String>,
        /// Max results
        #[arg(long, default_value = "20")]
        limit: usize,
        /// Search remote API instead of local vault
        #[arg(long)]
        remote: bool,
    },
    /// Short alias for post:list
    Ls {
        /// Filter by topic ID
        #[arg(long)]
        topic: Option<String>,
    },
    /// Show vault tree
    Tree,
    /// Pull posts from Slab to local vault
    Pull {
        /// Pull only a specific topic
        #[arg(long)]
        topic: Option<String>,
        /// Pull a single post by ID
        #[arg(long)]
        post: Option<String>,
        /// Pull all posts
        #[arg(long)]
        all: bool,
    },
    /// Show sync status (modified/added/deleted files)
    Status,
    /// Show diff between local and remote
    Diff {
        /// File to diff (default: all modified)
        file: Option<String>,
    },
    /// Push local changes to Slab
    Push {
        /// File or post to push (default: all modified)
        file: Option<String>,
        /// Push all modified files
        #[arg(long)]
        all: bool,
        /// Preview changes without pushing
        #[arg(long)]
        dry_run: bool,
        /// Overwrite remote even if it changed
        #[arg(long)]
        force: bool,
    },
    /// List comments on a post
    #[command(name = "comment:list")]
    CommentList {
        /// Post ID
        post_id: String,
    },
    /// Add a comment to a post
    #[command(name = "comment:add")]
    CommentAdd {
        /// Post ID
        post_id: String,
        /// Comment text
        body: String,
        /// Reply to an existing thread ID
        #[arg(long)]
        thread: Option<String>,
    },
    /// Update a comment
    #[command(name = "comment:update")]
    CommentUpdate {
        /// Comment ID
        comment_id: String,
        /// New comment text
        body: String,
    },
    /// Delete a comment
    #[command(name = "comment:delete")]
    CommentDelete {
        /// Comment ID
        comment_id: String,
    },
    /// Resolve a comment thread
    #[command(name = "comment:resolve")]
    CommentResolve {
        /// Thread ID to resolve
        thread_id: String,
    },
    /// React to a comment with an emoji
    #[command(name = "comment:react")]
    CommentReact {
        /// Comment ID
        comment_id: String,
        /// Emoji to react with (e.g. "👍")
        emoji: String,
    },
    /// Delete an entire comment thread
    #[command(name = "thread:delete")]
    ThreadDelete {
        /// Thread ID to delete
        thread_id: String,
    },
    /// Find links in a post
    Links {
        /// File to inspect (local vault path)
        file: Option<String>,
    },
    /// Find backlinks to a post
    Backlinks {
        /// File to inspect (local vault path)
        file: Option<String>,
    },
    /// Open a post in the browser
    Open {
        /// Post ID, URL, or local path
        id: String,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: clap_complete::Shell,
    },
}

fn main() {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    let format = if cli.json {
        output::Format::Json
    } else {
        cli.format
    };
    let ctx = cmd::Context {
        team: cli.team,
        token: cli.token,
        vault: cli.vault,
        format,
        copy: cli.copy,
    };

    let result = run(ctx, cli.command);
    if let Err(e) = result {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(ctx: cmd::Context, command: Command) -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    match command {
        Command::Auth { action } => cmd::auth::run(&ctx, action),
        Command::Vault { action } => cmd::vault_cmd::run(&ctx, action),
        Command::PostGet { id, raw } => rt.block_on(cmd::post::get(&ctx, &id, raw)),
        Command::PostList { topic } => rt.block_on(cmd::post::list(&ctx, topic.as_deref())),
        Command::PostCreate {
            title,
            topic,
            content_file,
        } => rt.block_on(cmd::post::create(
            &ctx,
            &title,
            topic.as_deref(),
            content_file.as_deref(),
        )),
        Command::PostUpdate {
            id,
            content_file,
            title,
        } => rt.block_on(cmd::post::update(
            &ctx,
            &id,
            content_file.as_deref(),
            title.as_deref(),
        )),
        Command::TopicList => rt.block_on(cmd::topic::list(&ctx)),
        Command::TopicGet { id } => rt.block_on(cmd::topic::get(&ctx, &id)),
        Command::TopicPosts { id } => rt.block_on(cmd::topic::posts(&ctx, &id)),
        Command::Search {
            query,
            topic: _,
            limit,
            remote,
        } => {
            if remote {
                rt.block_on(cmd::search::remote(&ctx, &query, limit))
            } else {
                cmd::search::local(&ctx, &query, limit)
            }
        }
        Command::Ls { topic } => rt.block_on(cmd::post::list(&ctx, topic.as_deref())),
        Command::Tree => cmd::tree::run(&ctx),
        Command::Pull { topic, post, all } => rt.block_on(cmd::sync::pull(
            &ctx,
            topic.as_deref(),
            post.as_deref(),
            all,
        )),
        Command::Status => cmd::sync::status(&ctx),
        Command::Diff { file } => cmd::sync::diff(&ctx, file.as_deref()),
        Command::Push {
            file,
            all,
            dry_run,
            force,
        } => rt.block_on(cmd::sync::push(&ctx, file.as_deref(), all, dry_run, force)),
        Command::CommentList { post_id } => rt.block_on(cmd::comment::list(&ctx, &post_id)),
        Command::CommentAdd {
            post_id,
            body,
            thread,
        } => rt.block_on(cmd::comment::add(&ctx, &post_id, &body, thread.as_deref())),
        Command::CommentUpdate { comment_id, body } => {
            rt.block_on(cmd::comment::update(&ctx, &comment_id, &body))
        }
        Command::CommentDelete { comment_id } => {
            rt.block_on(cmd::comment::delete(&ctx, &comment_id))
        }
        Command::CommentResolve { thread_id } => {
            rt.block_on(cmd::comment::resolve(&ctx, &thread_id))
        }
        Command::CommentReact { comment_id, emoji } => {
            rt.block_on(cmd::comment::react(&ctx, &comment_id, &emoji))
        }
        Command::ThreadDelete { thread_id } => {
            rt.block_on(cmd::comment::delete_thread(&ctx, &thread_id))
        }
        Command::Links { file } => cmd::links::links(&ctx, file.as_deref()),
        Command::Backlinks { file } => cmd::links::backlinks(&ctx, file.as_deref()),
        Command::Open { id } => cmd::open::run(&ctx, &id),
        Command::Completions { shell } => {
            clap_complete::generate(
                shell,
                &mut <Cli as clap::CommandFactory>::command(),
                "slab",
                &mut std::io::stdout(),
            );
            Ok(())
        }
    }
}
