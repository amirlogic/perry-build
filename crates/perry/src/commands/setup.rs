//! `perry setup` — guided credential setup wizard for App Store / Google Play distribution

use anyhow::{bail, Context, Result};
use clap::Args;
use console::style;
use dialoguer::{Confirm, Input, Password, Select};
use std::process::Command;

use super::publish::{
    AndroidSavedConfig, AppleSavedConfig, PerryConfig,
    config_path, is_interactive, load_config, save_config,
};

#[derive(Args, Debug)]
pub struct SetupArgs {
    /// Platform to configure: android, ios, macos
    pub platform: Option<String>,
}

pub fn run(args: SetupArgs) -> Result<()> {
    if !is_interactive() {
        bail!("`perry setup` requires an interactive terminal.");
    }

    println!();
    println!("  {} Perry Setup", style("▶").cyan().bold());
    println!();

    let platform = match args.platform.as_deref() {
        Some(p) => p.to_string(),
        None => {
            let options = &["Android", "iOS", "macOS"];
            let selection = Select::new()
                .with_prompt("  Which platform to configure?")
                .items(options)
                .default(0)
                .interact()?;
            match selection {
                0 => "android".to_string(),
                1 => "ios".to_string(),
                _ => "macos".to_string(),
            }
        }
    };

    let mut saved = load_config();

    match platform.as_str() {
        "android" => android_wizard(&mut saved)?,
        "ios" => ios_wizard(&mut saved)?,
        "macos" => macos_wizard(&mut saved)?,
        other => bail!("Unknown platform '{other}'. Use: android, ios, macos"),
    }

    save_config(&saved)?;
    println!();
    println!(
        "  {} Configuration saved to {}",
        style("✓").green().bold(),
        style(config_path().display()).dim()
    );
    println!();
    Ok(())
}

// ---------------------------------------------------------------------------
// Android wizard
// ---------------------------------------------------------------------------

fn android_wizard(saved: &mut PerryConfig) -> Result<()> {
    println!("  {}", style("Android Setup").bold());
    println!();

    // --- Step 1: Keystore ---
    println!("  {} Keystore", style("Step 1/2 —").cyan().bold());
    println!();

    let has_keystore = Confirm::new()
        .with_prompt("  Do you have an existing Android keystore?")
        .default(true)
        .interact()?;

    let (keystore_path, key_alias) = if has_keystore {
        let path = Input::<String>::new()
            .with_prompt("  Keystore path")
            .interact_text()?;
        let path = expand_tilde(&path);
        let alias = Input::<String>::new()
            .with_prompt("  Key alias")
            .default("key0".to_string())
            .interact_text()?;
        if !std::path::Path::new(&path).exists() {
            bail!("Keystore file not found: {path}");
        }
        (path, alias)
    } else {
        // Check for keytool
        if std::process::Command::new("keytool")
            .arg("-help")
            .stderr(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .status()
            .is_err()
        {
            bail!(
                "keytool not found — install a JDK first (e.g. brew install --cask temurin) \
                 and try again."
            );
        }

        println!("  Generating a new Android release keystore...");
        println!();

        let path = Input::<String>::new()
            .with_prompt("  Output path (e.g. ~/release-key.keystore)")
            .interact_text()?;
        let path = expand_tilde(&path);
        let alias = Input::<String>::new()
            .with_prompt("  Key alias")
            .default("key0".to_string())
            .interact_text()?;
        let password = Password::new()
            .with_prompt("  Keystore password")
            .with_confirmation("  Confirm password", "Passwords didn't match")
            .interact()?;

        let status = std::process::Command::new("keytool")
            .args([
                "-genkeypair",
                "-v",
                "-keystore",
                &path,
                "-keyalg",
                "RSA",
                "-keysize",
                "2048",
                "-validity",
                "10000",
                "-alias",
                &alias,
                "-storepass",
                &password,
                "-keypass",
                &password,
                "-dname",
                "CN=Android, O=Android, C=US",
            ])
            .status()?;

        if !status.success() {
            bail!("keytool failed to generate keystore");
        }

        println!();
        println!("  {} Keystore created at {}", style("✓").green(), style(&path).bold());
        (path, alias)
    };

    let android = saved.android.get_or_insert_with(AndroidSavedConfig::default);
    android.keystore_path = Some(keystore_path.clone());
    android.key_alias = Some(key_alias.clone());

    println!("  {} Keystore: {}", style("✓").green(), style(&keystore_path).bold());
    println!("  {} Key alias: {}", style("✓").green(), style(&key_alias).bold());
    println!();

    // --- Step 2: Google Play Service Account ---
    println!("  {} Google Play Service Account", style("Step 2/2 —").cyan().bold());
    println!();
    println!("  Follow these steps to enable automated Play Store uploads:");
    println!();
    println!("  1. Open Play Console → Setup → API access:");
    println!("     https://play.google.com/console/developers/api-access");
    println!("     Link your console to a Google Cloud project.");
    println!();
    println!("  2. Create a service account + download its JSON key:");
    println!("     https://console.cloud.google.com/iam-admin/serviceaccounts");
    println!("     → Create Service Account → Add Key → Create new key → JSON");
    println!();
    println!("  3. Back in Play Console → Users & Permissions → Invite user");
    println!("     Add the service account email with Release Manager permissions.");
    println!();
    println!(
        "  {} The first release MUST be uploaded manually via Play Console before",
        style("!").yellow()
    );
    println!("     automated uploads will work.");
    println!();

    press_enter_to_continue("  Press Enter when ready");

    let json_path = Input::<String>::new()
        .with_prompt("  Path to service account JSON key")
        .interact_text()?;
    let json_path = expand_tilde(&json_path);

    if !std::path::Path::new(&json_path).exists() {
        bail!("Service account JSON not found: {json_path}");
    }

    // Validate JSON content
    let json_content = std::fs::read_to_string(&json_path)?;
    let parsed: serde_json::Value =
        serde_json::from_str(&json_content).map_err(|e| anyhow::anyhow!("Invalid JSON: {e}"))?;
    let client_email = parsed["client_email"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'client_email' in service account JSON"))?;
    if parsed["private_key"].as_str().is_none() {
        bail!("Missing 'private_key' in service account JSON");
    }

    println!(
        "  {} Service account: {}",
        style("✓").green(),
        style(client_email).bold()
    );

    let android = saved.android.get_or_insert_with(AndroidSavedConfig::default);
    android.google_play_key_path = Some(json_path);

    println!();
    println!("  Add to your perry.toml:");
    println!();
    println!("  {}", style("[android]").cyan());
    println!("  distribute = \"playstore\"");
    println!();
    println!("  Tip: to target a specific track, use:");
    println!("  distribute = \"playstore:beta\"  {} :internal, :alpha, :beta, :production", style("#").dim());

    Ok(())
}

// ---------------------------------------------------------------------------
// iOS wizard
// ---------------------------------------------------------------------------

fn ios_wizard(saved: &mut PerryConfig) -> Result<()> {
    println!("  {}", style("iOS Setup").bold());
    println!("  Automates: certificate, bundle ID, and provisioning profile via App Store Connect API");
    println!();

    // --- Step 1: App Store Connect API Key ---
    // Check for existing credentials first
    let existing_apple = saved.apple.clone().unwrap_or_default();

    println!("  {} App Store Connect API Key", style("Step 1 —").cyan().bold());
    println!();

    let has_existing = existing_apple.p8_key_path.is_some()
        && existing_apple.key_id.is_some()
        && existing_apple.issuer_id.is_some();

    let (p8_path, key_id, issuer_id, team_id) = if has_existing {
        let p8 = existing_apple.p8_key_path.clone().unwrap();
        let kid = existing_apple.key_id.clone().unwrap();
        let iss = existing_apple.issuer_id.clone().unwrap();
        let tid = existing_apple.team_id.clone().unwrap_or_default();
        println!("  Found existing credentials:");
        println!("    Key ID:    {}", style(&kid).bold());
        println!("    Issuer ID: {}", style(&iss).dim());
        println!("    .p8 key:   {}", style(&p8).dim());
        println!();
        let reuse = Confirm::new()
            .with_prompt("  Use these existing credentials?")
            .default(true)
            .interact()?;
        if reuse {
            (p8, kid, iss, tid)
        } else {
            prompt_api_credentials()?
        }
    } else {
        println!("  You need an App Store Connect API key.");
        println!("  1. Go to: {}", style("https://appstoreconnect.apple.com/access/integrations/api").underlined());
        println!("  2. Click '+', create a key with {} role.", style("App Manager").bold());
        println!("  3. Download the .p8 file (only downloadable once).");
        println!("  4. Note the Key ID and Issuer ID.");
        println!();
        press_enter_to_continue("  Press Enter when ready");
        prompt_api_credentials()?
    };

    // Validate p8 file
    let p8_content = std::fs::read_to_string(&p8_path)
        .with_context(|| format!("Cannot read .p8 key: {p8_path}"))?;
    if !p8_content.trim_start().starts_with("-----BEGIN") {
        bail!("Invalid .p8 file — expected PEM format");
    }

    // Save API credentials immediately
    let apple = saved.apple.get_or_insert_with(AppleSavedConfig::default);
    apple.p8_key_path = Some(p8_path.clone());
    apple.key_id = Some(key_id.clone());
    apple.issuer_id = Some(issuer_id.clone());
    apple.team_id = Some(team_id.clone());
    save_config(saved).ok();

    println!("  {} API credentials configured", style("✓").green().bold());
    println!();

    // Generate JWT for API calls
    let jwt = generate_asc_jwt(&key_id, &issuer_id, &p8_content)?;

    // Verify API connectivity
    print!("  Verifying API access... ");
    std::io::Write::flush(&mut std::io::stdout()).ok();
    let client = reqwest::blocking::Client::new();
    let resp = client.get("https://api.appstoreconnect.apple.com/v1/certificates?limit=1")
        .bearer_auth(&jwt)
        .send()
        .context("Failed to connect to App Store Connect API")?;
    if resp.status() == 401 || resp.status() == 403 {
        bail!("API authentication failed — check your Key ID, Issuer ID, and .p8 key file");
    }
    if !resp.status().is_success() {
        let body = resp.text().unwrap_or_default();
        bail!("API error: {body}");
    }
    println!("{}", style("ok").green());
    println!();

    // --- Step 2: Read bundle_id from perry.toml ---
    let perry_toml_path = std::env::current_dir()?.join("perry.toml");
    let bundle_id = if perry_toml_path.exists() {
        let content = std::fs::read_to_string(&perry_toml_path)?;
        let parsed: toml::Value = toml::from_str(&content)?;
        parsed.get("project")
            .and_then(|p| p.get("bundle_id"))
            .and_then(|v| v.as_str())
            .or_else(|| parsed.get("ios").and_then(|i| i.get("bundle_id")).and_then(|v| v.as_str()))
            .map(|s| s.to_string())
    } else {
        None
    };

    let bundle_id = if let Some(bid) = bundle_id {
        println!("  Found bundle ID in perry.toml: {}", style(&bid).bold());
        let use_it = Confirm::new()
            .with_prompt("  Use this bundle ID?")
            .default(true)
            .interact()?;
        if use_it { bid } else {
            Input::<String>::new()
                .with_prompt("  Bundle ID (e.g. com.company.app)")
                .interact_text()?
        }
    } else {
        Input::<String>::new()
            .with_prompt("  Bundle ID (e.g. com.company.app)")
            .interact_text()?
    };
    println!();

    // --- Step 3: Register Bundle ID (App ID) if needed ---
    println!("  {} Registering App ID", style("Step 2 —").cyan().bold());
    print!("  Checking if {} exists... ", style(&bundle_id).bold());
    std::io::Write::flush(&mut std::io::stdout()).ok();

    let jwt = generate_asc_jwt(&key_id, &issuer_id, &p8_content)?;
    let resp = client.get("https://api.appstoreconnect.apple.com/v1/bundleIds")
        .bearer_auth(&jwt)
        .query(&[("filter[identifier]", &bundle_id), ("limit", &"1".to_string())])
        .send()?;
    let body: serde_json::Value = resp.json()?;
    let existing_bundle_ids = body["data"].as_array();
    let bundle_id_resource_id = if let Some(ids) = existing_bundle_ids {
        if ids.is_empty() {
            println!("{}", style("not found, creating...").yellow());
            // Register new bundle ID
            let jwt = generate_asc_jwt(&key_id, &issuer_id, &p8_content)?;
            let app_name = bundle_id.split('.').last().unwrap_or("app");
            let create_body = serde_json::json!({
                "data": {
                    "type": "bundleIds",
                    "attributes": {
                        "identifier": bundle_id,
                        "name": format!("Perry - {}", app_name),
                        "platform": "IOS"
                    }
                }
            });
            let resp = client.post("https://api.appstoreconnect.apple.com/v1/bundleIds")
                .bearer_auth(&jwt)
                .json(&create_body)
                .send()?;
            if !resp.status().is_success() {
                let err = resp.text().unwrap_or_default();
                bail!("Failed to register Bundle ID: {err}");
            }
            let resp_body: serde_json::Value = resp.json()?;
            let rid = resp_body["data"]["id"].as_str()
                .ok_or_else(|| anyhow::anyhow!("No ID in bundle registration response"))?
                .to_string();
            println!("  {} Registered: {}", style("✓").green().bold(), style(&bundle_id).bold());
            rid
        } else {
            println!("{}", style("exists").green());
            ids[0]["id"].as_str().unwrap_or("").to_string()
        }
    } else {
        bail!("Unexpected API response when checking bundle IDs");
    };
    println!();

    // --- Step 4: Create or find Distribution Certificate ---
    println!("  {} Distribution Certificate", style("Step 3 —").cyan().bold());
    print!("  Checking for existing distribution certificates... ");
    std::io::Write::flush(&mut std::io::stdout()).ok();

    let jwt = generate_asc_jwt(&key_id, &issuer_id, &p8_content)?;
    let resp = client.get("https://api.appstoreconnect.apple.com/v1/certificates")
        .bearer_auth(&jwt)
        .query(&[("filter[certificateType]", "DISTRIBUTION"), ("limit", "200")])
        .send()?;
    let body: serde_json::Value = resp.json()?;
    let certs = body["data"].as_array();

    let perry_dir = dirs::home_dir().unwrap_or_default().join(".perry");
    std::fs::create_dir_all(&perry_dir)?;
    let p12_path = perry_dir.join("distribution.p12");
    let p12_password = "perry-auto";

    // Check if we already have a valid .p12 with matching cert
    // Collect ALL valid distribution cert IDs — profile will include all of them
    let all_cert_ids: Vec<String> = if let Some(cert_list) = certs {
        let valid: Vec<String> = cert_list.iter()
            .filter(|c| c["attributes"]["certificateType"].as_str() == Some("DISTRIBUTION"))
            .filter_map(|c| c["id"].as_str().map(|s| s.to_string()))
            .collect();
        if valid.is_empty() {
            println!("{}", style("none found").yellow());
        } else {
            println!("{} found", style(format!("{}", valid.len())).green());
        }
        valid
    } else {
        println!("{}", style("error reading").red());
        vec![]
    };

    let existing_cert_id = if !all_cert_ids.is_empty() && p12_path.exists() {
        println!("  Found existing .p12 at {}", style(p12_path.display()).dim());
        let keep = Confirm::new()
            .with_prompt("  Keep existing certificate?")
            .default(true)
            .interact()?;
        if keep {
            Some(all_cert_ids[0].clone()) // placeholder — profile will use all certs
        } else {
            None
        }
    } else if !all_cert_ids.is_empty() {
        Some(all_cert_ids[0].clone())
    } else {
        None
    };

    let mut created_signing_identity: Option<String> = None;
    let cert_resource_id = if let Some(id) = existing_cert_id {
        id
    } else {
        // Generate a new private key + CSR, submit to Apple, get cert back, make .p12
        println!("  Generating private key and certificate signing request...");
        let key_path = perry_dir.join("dist_private_key.pem");
        let csr_path = perry_dir.join("dist_csr.pem");

        // Generate RSA 2048 private key
        let status = Command::new("openssl")
            .args(["genrsa", "-out"])
            .arg(&key_path)
            .arg("2048")
            .stderr(std::process::Stdio::null())
            .status()
            .context("openssl not found — required for certificate generation")?;
        if !status.success() {
            bail!("Failed to generate private key");
        }

        // Generate CSR
        let status = Command::new("openssl")
            .args(["req", "-new", "-key"])
            .arg(&key_path)
            .args(["-out"])
            .arg(&csr_path)
            .args(["-subj", "/CN=Perry Distribution/O=Perry"])
            .stderr(std::process::Stdio::null())
            .status()?;
        if !status.success() {
            bail!("Failed to generate CSR");
        }

        // Read CSR as DER (base64)
        let csr_pem = std::fs::read_to_string(&csr_path)?;
        let csr_b64: String = csr_pem.lines()
            .filter(|l| !l.starts_with("-----"))
            .collect::<Vec<_>>()
            .join("");

        // Submit CSR to Apple
        print!("  Submitting certificate request to Apple... ");
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let jwt = generate_asc_jwt(&key_id, &issuer_id, &p8_content)?;
        let create_body = serde_json::json!({
            "data": {
                "type": "certificates",
                "attributes": {
                    "certificateType": "DISTRIBUTION",
                    "csrContent": csr_b64
                }
            }
        });
        let resp = client.post("https://api.appstoreconnect.apple.com/v1/certificates")
            .bearer_auth(&jwt)
            .json(&create_body)
            .send()?;
        if !resp.status().is_success() {
            let err = resp.text().unwrap_or_default();
            bail!("Failed to create certificate: {err}");
        }
        let resp_body: serde_json::Value = resp.json()?;
        let cert_content_b64 = resp_body["data"]["attributes"]["certificateContent"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No certificate content in response"))?;
        let cert_id = resp_body["data"]["id"].as_str().unwrap_or("").to_string();
        let cert_name = resp_body["data"]["attributes"]["name"].as_str().unwrap_or("Unknown");
        println!("{}", style("done").green());
        println!("  {} Certificate: {}", style("✓").green().bold(), style(cert_name).bold());

        // Decode cert and write as PEM
        use base64::Engine;
        let cert_der = base64::engine::general_purpose::STANDARD.decode(cert_content_b64)
            .context("Failed to decode certificate from Apple")?;
        let cert_pem_path = perry_dir.join("distribution.cer.pem");
        let cert_pem = format!(
            "-----BEGIN CERTIFICATE-----\n{}\n-----END CERTIFICATE-----\n",
            base64::engine::general_purpose::STANDARD.encode(&cert_der)
                .as_bytes()
                .chunks(76)
                .map(|c| std::str::from_utf8(c).unwrap_or(""))
                .collect::<Vec<_>>()
                .join("\n")
        );
        std::fs::write(&cert_pem_path, &cert_pem)?;

        // Create .p12 from private key + certificate
        print!("  Creating .p12 bundle... ");
        std::io::Write::flush(&mut std::io::stdout()).ok();

        let status = Command::new("openssl")
            .args(["pkcs12", "-export",
                   "-inkey"])
            .arg(&key_path)
            .args(["-in"])
            .arg(&cert_pem_path)
            .args(["-out"])
            .arg(&p12_path)
            .args(["-password", &format!("pass:{p12_password}"),
                   "-legacy"]) // macOS openssl compatibility
            .stderr(std::process::Stdio::null())
            .status()?;
        if !status.success() {
            // Try without -legacy flag (older openssl)
            let status = Command::new("openssl")
                .args(["pkcs12", "-export",
                       "-inkey"])
                .arg(&key_path)
                .args(["-in"])
                .arg(&cert_pem_path)
                .args(["-out"])
                .arg(&p12_path)
                .args(["-password", &format!("pass:{p12_password}")])
                .stderr(std::process::Stdio::null())
                .status()?;
            if !status.success() {
                bail!("Failed to create .p12 certificate bundle");
            }
        }
        println!("{}", style("done").green());

        // Derive signing identity from cert (will be saved to perry.toml, not global config)
        let identity = format!("Apple Distribution: {} ({})",
            cert_name.strip_prefix("Apple Distribution: ").unwrap_or(cert_name),
            &team_id);
        println!("  {} Identity: {}", style("✓").green().bold(), style(&identity).bold());
        created_signing_identity = Some(identity);

        // Clean up intermediate files (keep the private key for potential re-use)
        let _ = std::fs::remove_file(&csr_path);
        let _ = std::fs::remove_file(&cert_pem_path);

        cert_id
    };

    save_config(saved).ok();
    println!();

    // --- Step 5: Create Provisioning Profile ---
    println!("  {} Provisioning Profile", style("Step 4 —").cyan().bold());
    print!("  Creating provisioning profile for {}... ", style(&bundle_id).bold());
    std::io::Write::flush(&mut std::io::stdout()).ok();

    let jwt = generate_asc_jwt(&key_id, &issuer_id, &p8_content)?;

    // First check if one already exists
    let resp = client.get("https://api.appstoreconnect.apple.com/v1/profiles")
        .bearer_auth(&jwt)
        .query(&[("filter[profileType]", "IOS_APP_STORE"), ("include", "bundleId"), ("limit", "200")])
        .send()?;
    let body: serde_json::Value = resp.json()?;
    let existing_profile = body["data"].as_array()
        .and_then(|profiles| {
            profiles.iter().find(|p| {
                // Check if this profile's bundle ID matches ours
                let bid_id = p["relationships"]["bundleId"]["data"]["id"].as_str().unwrap_or("");
                bid_id == bundle_id_resource_id
            })
        });

    let profile_b64 = if let Some(profile) = existing_profile {
        // Delete existing profile and recreate — it may reference an old certificate
        let profile_id = profile["id"].as_str().unwrap_or("");
        if !profile_id.is_empty() {
            print!("{}, replacing... ", style("found existing").yellow());
            std::io::Write::flush(&mut std::io::stdout()).ok();
            let jwt = generate_asc_jwt(&key_id, &issuer_id, &p8_content)?;
            let _ = client.delete(format!("https://api.appstoreconnect.apple.com/v1/profiles/{profile_id}"))
                .bearer_auth(&jwt)
                .send();
        }
        // Fall through to create new profile below
        "".to_string()
    } else {
        "".to_string()
    };
    let profile_b64 = if profile_b64.is_empty() {
        // Create new profile
        let create_body = serde_json::json!({
            "data": {
                "type": "profiles",
                "attributes": {
                    "name": format!("Perry - {}", bundle_id),
                    "profileType": "IOS_APP_STORE"
                },
                "relationships": {
                    "bundleId": {
                        "data": {
                            "type": "bundleIds",
                            "id": bundle_id_resource_id
                        }
                    },
                    "certificates": {
                        "data": all_cert_ids.iter().map(|id| {
                            serde_json::json!({"type": "certificates", "id": id})
                        }).collect::<Vec<_>>()
                    }
                }
            }
        });
        let jwt = generate_asc_jwt(&key_id, &issuer_id, &p8_content)?;
        let resp = client.post("https://api.appstoreconnect.apple.com/v1/profiles")
            .bearer_auth(&jwt)
            .json(&create_body)
            .send()?;
        if !resp.status().is_success() {
            let err = resp.text().unwrap_or_default();
            bail!("Failed to create provisioning profile: {err}");
        }
        let resp_body: serde_json::Value = resp.json()?;
        println!("{}", style("created").green());
        resp_body["data"]["attributes"]["profileContent"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("No profile content in response"))?
            .to_string()
    } else {
        profile_b64
    };

    // Decode and save the provisioning profile
    use base64::Engine;
    let profile_data = base64::engine::general_purpose::STANDARD.decode(&profile_b64)
        .context("Failed to decode provisioning profile")?;
    let profile_path = perry_dir.join("perry.mobileprovision");
    std::fs::write(&profile_path, &profile_data)?;

    println!("  {} Profile saved to {}", style("✓").green().bold(), style(profile_path.display()).dim());
    println!();

    // --- Save project-specific credentials to perry.toml ---
    let p12_str = p12_path.to_string_lossy().to_string();
    let profile_str = profile_path.to_string_lossy().to_string();

    if perry_toml_path.exists() {
        match update_perry_toml_ios(
            &perry_toml_path,
            &p12_str,
            &profile_str,
            created_signing_identity.as_deref(),
        ) {
            Ok(()) => {
                println!("  {} Project credentials saved to {}", style("✓").green().bold(),
                    style(perry_toml_path.display()).dim());
            }
            Err(e) => {
                println!("  {} Could not update perry.toml: {e}", style("!").yellow());
                println!("  Add these manually to your perry.toml [ios] section:");
                println!("  certificate = \"{}\"", p12_str);
                println!("  provisioning_profile = \"{}\"", profile_str);
            }
        }
    } else {
        println!("  Add these to your perry.toml [ios] section:");
        println!("  certificate = \"{}\"", p12_str);
        println!("  provisioning_profile = \"{}\"", profile_str);
    }
    println!();

    // --- Summary ---
    println!("  {}", style("Setup complete!").green().bold());
    println!();
    println!("  Certificate:  {}", style(p12_path.display()).dim());
    println!("  Profile:      {}", style(profile_path.display()).dim());
    println!("  Cert password: {}", style(p12_password).bold());
    println!();
    println!("  Set the password in your environment:");
    println!("  export PERRY_APPLE_CERTIFICATE_PASSWORD={p12_password}");
    println!();
    println!("  Then run: {}", style("perry publish --ios").bold());

    Ok(())
}

/// Generate an App Store Connect API JWT token (ES256, 20-minute expiry)
fn generate_asc_jwt(key_id: &str, issuer_id: &str, p8_content: &str) -> Result<String> {
    use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let header = Header {
        alg: Algorithm::ES256,
        kid: Some(key_id.to_string()),
        typ: Some("JWT".to_string()),
        ..Default::default()
    };

    let claims = serde_json::json!({
        "iss": issuer_id,
        "iat": now,
        "exp": now + 1200,
        "aud": "appstoreconnect-v1"
    });

    let encoding_key = EncodingKey::from_ec_pem(p8_content.as_bytes())
        .context("Failed to parse .p8 key — ensure it's a valid EC private key")?;

    let token = encode(&header, &claims, &encoding_key)
        .context("Failed to generate JWT")?;

    Ok(token)
}

/// Prompt for App Store Connect API credentials
fn prompt_api_credentials() -> Result<(String, String, String, String)> {
    let p8_path = prompt_file_path("  Path to .p8 key file", ".p8")?;
    let key_id = Input::<String>::new()
        .with_prompt("  Key ID (e.g. ABC123XYZ)")
        .interact_text()?;
    let issuer_id = Input::<String>::new()
        .with_prompt("  Issuer ID (UUID format)")
        .interact_text()?;
    let team_id = Input::<String>::new()
        .with_prompt("  Apple Developer Team ID (10 chars)")
        .interact_text()?;
    Ok((p8_path, key_id, issuer_id, team_id))
}

// ---------------------------------------------------------------------------
// macOS wizard
// ---------------------------------------------------------------------------

fn macos_wizard(saved: &mut PerryConfig) -> Result<()> {
    println!("  {}", style("macOS Setup").bold());
    println!();

    // --- Step 1: App Store Connect API Key ---
    println!("  {} App Store Connect API Key", style("Step 1/2 —").cyan().bold());
    println!();
    println!("  1. Go to App Store Connect → Users and Access → Integrations → API:");
    println!("     https://appstoreconnect.apple.com/access/integrations/api");
    println!("  2. Click '+' and create a key with App Manager role.");
    println!("  3. Download the .p8 file immediately — it can only be downloaded once.");
    println!("  4. Note the Key ID and Issuer ID shown on the page.");
    println!();

    press_enter_to_continue("  Press Enter when ready");

    let p8_path = prompt_file_path("  Path to .p8 key file", ".p8")?;
    let p8_content = std::fs::read_to_string(&p8_path)?;
    if !p8_content.trim_start().starts_with("-----BEGIN") {
        bail!("Invalid .p8 file — expected PEM format starting with '-----BEGIN'");
    }

    let key_id = Input::<String>::new()
        .with_prompt("  Key ID (e.g. ABC123XYZ)")
        .interact_text()?;
    let issuer_id = Input::<String>::new()
        .with_prompt("  Issuer ID (UUID format, e.g. a1b2c3d4-...)")
        .interact_text()?;
    let team_id = Input::<String>::new()
        .with_prompt("  Apple Developer Team ID (10 characters)")
        .interact_text()?;

    let apple = saved.apple.get_or_insert_with(AppleSavedConfig::default);
    apple.p8_key_path = Some(p8_path.clone());
    apple.key_id = Some(key_id.clone());
    apple.issuer_id = Some(issuer_id.clone());
    apple.team_id = Some(team_id.clone());

    println!();
    println!("  {} Key ID: {}", style("✓").green(), style(&key_id).bold());
    println!("  {} Issuer ID: {}", style("✓").green(), style(&issuer_id).bold());
    println!("  {} Team ID: {}", style("✓").green(), style(&team_id).bold());
    println!();

    // --- Step 2: Mac Distribution Certificate ---
    println!("  {} Mac Distribution Certificate", style("Step 2/2 —").cyan().bold());
    println!();

    let cert_types = &["Mac App Store (submit to App Store)", "Developer ID (direct distribution / notarize)"];
    let cert_type_idx = Select::new()
        .with_prompt("  Distribution method")
        .items(cert_types)
        .default(0)
        .interact()?;
    let is_appstore = cert_type_idx == 0;

    println!();
    if is_appstore {
        println!("  To create a Mac App Store distribution certificate:");
        println!("  1. Open Xcode → Settings → Accounts → select your Apple ID.");
        println!("  2. Click 'Manage Certificates'.");
        println!("  3. Create a 'Mac App Distribution' certificate.");
        println!("  4. Right-click → Export Certificate → save as .p12.");
    } else {
        println!("  To create a Developer ID Application certificate:");
        println!("  1. Open Xcode → Settings → Accounts → select your Apple ID.");
        println!("  2. Click 'Manage Certificates'.");
        println!("  3. Create a 'Developer ID Application' certificate.");
        println!("  4. Right-click → Export Certificate → save as .p12.");
    }
    println!();

    press_enter_to_continue("  Press Enter when ready");

    let cert_path = prompt_file_path("  Path to .p12 certificate", ".p12")?;

    let signing_identity = Input::<String>::new()
        .with_prompt("  Signing identity string (optional, e.g. 'Developer ID Application: ...')")
        .allow_empty(true)
        .interact_text()?;

    println!("  {} Certificate: {}", style("✓").green(), style(&cert_path).bold());
    println!(
        "  {} Certificate password is NOT saved — set PERRY_APPLE_CERTIFICATE_PASSWORD",
        style("ℹ").blue()
    );
    println!();

    let distribute_value = if is_appstore { "appstore" } else { "notarize" };
    println!("  Add to your perry.toml:");
    println!();
    println!("  {}", style("[macos]").cyan());
    println!("  distribute = \"{distribute_value}\"");
    println!("  certificate = \"{}\"", cert_path);
    if !signing_identity.is_empty() {
        println!("  signing_identity = \"{}\"", signing_identity);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Expand leading `~/` to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if path.starts_with("~/") {
        dirs::home_dir()
            .map(|h| h.join(&path[2..]).to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string())
    } else {
        path.to_string()
    }
}

/// Prompt for a file path, validate it exists and has the expected extension.
fn prompt_file_path(prompt: &str, expected_ext: &str) -> Result<String> {
    let path = Input::<String>::new()
        .with_prompt(prompt)
        .interact_text()?;
    let path = expand_tilde(&path);
    if !std::path::Path::new(&path).exists() {
        bail!("File not found: {path}");
    }
    if !path.ends_with(expected_ext) {
        bail!("Expected a {expected_ext} file, got: {path}");
    }
    Ok(path)
}

/// Display a "Press Enter to continue" prompt.
fn press_enter_to_continue(prompt: &str) {
    let _ = Input::<String>::new()
        .with_prompt(prompt)
        .allow_empty(true)
        .interact_text();
}

/// Update perry.toml [ios] section with project-specific signing credentials.
fn update_perry_toml_ios(
    perry_toml_path: &std::path::Path,
    certificate: &str,
    provisioning_profile: &str,
    signing_identity: Option<&str>,
) -> Result<()> {
    let content = std::fs::read_to_string(perry_toml_path)?;
    let mut doc = content.parse::<toml::Table>()
        .context("Failed to parse perry.toml")?;

    let ios = doc.entry("ios")
        .or_insert_with(|| toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .ok_or_else(|| anyhow::anyhow!("[ios] in perry.toml is not a table"))?;

    ios.insert("certificate".into(), toml::Value::String(certificate.into()));
    ios.insert("provisioning_profile".into(), toml::Value::String(provisioning_profile.into()));
    if let Some(identity) = signing_identity {
        ios.insert("signing_identity".into(), toml::Value::String(identity.into()));
    }

    let new_content = toml::to_string_pretty(&doc)
        .context("Failed to serialize perry.toml")?;
    std::fs::write(perry_toml_path, new_content)?;
    Ok(())
}
