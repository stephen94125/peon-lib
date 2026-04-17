use log::info;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use std::path::Path;

/// Initializes the local workspace for a Peon Agent.
/// 
/// This creates the necessary `./skills/` directory, an empty `.env` file, 
/// and populates the default `file_permissions.txt` and `user_permissions.csv` 
/// with a fully open "ALLOW ALL" policy for immediate testing.
///
/// It also prints detailed instructional logs to standard output.
pub async fn init_workspace() -> anyhow::Result<()> {
    info!("🚀 Initializing Peon Workspace...");

    // 1. Create skills directory
    if !Path::new("skills").exists() {
        tokio::fs::create_dir_all("skills").await?;
        info!("✅ Created directory: ./skills/");
    } else {
        info!("⏭️  Directory ./skills/ already exists, skipping.");
    }

    // 2. Create empty .env file
    if !Path::new(".env").exists() {
        File::create(".env").await?;
        info!("✅ Created file: .env (Empty, please configure your tokens here)");
    } else {
        info!("⏭️  File .env already exists, skipping.");
    }

    // 3. Create file_permissions.txt
    if !Path::new("file_permissions.txt").exists() {
        let mut file = File::create("file_permissions.txt").await?;
        let content = "r, /*\nw, /*\nx, /*\n";
        file.write_all(content.as_bytes()).await?;
        
        info!("✅ Created file: file_permissions.txt");
        println!("\n=======================================================");
        println!("📜 [File Permissions] (file_permissions.txt)");
        println!("An 'Allow All' policy has been injected by default. Please restrict this for production.");
        println!("Format per line: <action>, <target_path>");
        println!("  - `r`: Read access");
        println!("  - `w`: Write access");
        println!("  - `x`: Execute scripts or commands");
        println!("  - `!r`, `!w`, `!x`: Explicit Deny (Priority)");
        println!("\nExamples:");
        println!("  x, ./skills/*       # Allow execution within the skills directory");
        println!("  !r, ./secret.key    # Forcibly block access to secret.key");
        println!("=======================================================\n");
    } else {
        info!("⏭️  File file_permissions.txt already exists, skipping.");
    }

    // 4. Create user_permissions.csv
    if !Path::new("user_permissions.csv").exists() {
        let mut file = File::create("user_permissions.csv").await?;
        let content = "p, *, *, *, allow\n";
        file.write_all(content.as_bytes()).await?;

        info!("✅ Created file: user_permissions.csv");
        println!("\n=======================================================");
        println!("👥 [User Permissions] (user_permissions.csv)");
        println!("A global 'Allow All' policy has been injected for all users.");
        println!("Format per line: p, <subject/role>, <resource>, <action>, <effect>");
        println!("\nTo define roles or groups: g, <user>, <role>");
        println!("\nExamples:");
        println!("  p, *, *, *, allow                  # Allow everyone to do everything");
        println!("  p, Bob, *, execute, deny           # Block user Bob from any execution");
        println!("  p, admin_role, /root/*, *, allow   # Give admin_role access to /root");
        println!("  g, Alice, admin_role               # Assign Alice to the admin_role");
        println!("=======================================================\n");
    } else {
        info!("⏭️  File user_permissions.csv already exists, skipping.");
    }

    info!("🎉 Initialization complete. You are ready to start the agent!");
    
    Ok(())
}
