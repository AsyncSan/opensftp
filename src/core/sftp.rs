use std::sync::Arc;

use russh::client;
use russh::keys::PrivateKeyWithHashAlg;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

use super::connection::{AuthMethod, JumpHostConfig};
use super::known_hosts;

struct SshHandler {
    host: String,
    port: u16,
}

impl client::Handler for SshHandler {
    type Error = russh::Error;

    fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::PublicKey,
    ) -> impl std::future::Future<Output = Result<bool, Self::Error>> + Send {
        let result = known_hosts::check_host_key(&self.host, self.port, server_public_key);
        async move { Ok(result.is_ok()) }
    }
}

pub struct SftpConnection {
    pub sftp: SftpSession,
    handle: client::Handle<SshHandler>,
}

impl SftpConnection {
    pub async fn connect(
        host: &str,
        port: u16,
        username: &str,
        auth: &AuthMethod,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::connect_inner(host, port, username, auth, None).await
    }

    pub async fn connect_with_jump_host(
        host: &str,
        port: u16,
        username: &str,
        auth: &AuthMethod,
        jump_host: &JumpHostConfig,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        Self::connect_inner(host, port, username, auth, Some(jump_host)).await
    }

    async fn connect_inner(
        host: &str,
        port: u16,
        username: &str,
        auth: &AuthMethod,
        jump_host: Option<&JumpHostConfig>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let config = Arc::new(client::Config::default());
        let handle = if let Some(jump) = jump_host {
            let jump_handle = Self::connect_ssh(&config, &jump.host, jump.port, &jump.username, &jump.auth).await?;
            let channel = jump_handle.channel_open_direct_tcpip(host, port as u32, "127.0.0.1", 0).await?;
            let stream = channel.into_stream();
            client::connect_stream(
                config.clone(),
                stream,
                SshHandler {
                    host: host.to_string(),
                    port,
                },
            )
            .await?
        } else {
            let addr = format!("{}:{}", host, port);
            client::connect(
                config.clone(),
                &addr,
                SshHandler {
                    host: host.to_string(),
                    port,
                },
            )
            .await?
        };

        Self::authenticate_and_open_sftp(handle, username, auth).await
    }

    async fn connect_ssh(
        config: &Arc<client::Config>,
        host: &str,
        port: u16,
        username: &str,
        auth: &AuthMethod,
    ) -> Result<client::Handle<SshHandler>, Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("{}:{}", host, port);
        let mut handle = client::connect(
            config.clone(),
            &addr,
            SshHandler {
                host: host.to_string(),
                port,
            },
        )
        .await?;
        let auth_result: client::AuthResult = Self::authenticate(&mut handle, username, auth).await?;
        if !auth_result.success() {
            return Err("Authentication failed".into());
        }
        Ok(handle)
    }

    async fn authenticate_and_open_sftp(
        mut handle: client::Handle<SshHandler>,
        username: &str,
        auth: &AuthMethod,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let auth_result: client::AuthResult = Self::authenticate(&mut handle, username, auth).await?;
        if !auth_result.success() {
            return Err("Authentication failed".into());
        }
        let channel = handle.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        let sftp = SftpSession::new(channel.into_stream()).await?;
        Ok(Self { sftp, handle })
    }

    async fn authenticate(
        handle: &mut client::Handle<SshHandler>,
        username: &str,
        auth: &AuthMethod,
    ) -> Result<client::AuthResult, Box<dyn std::error::Error + Send + Sync>> {
        let auth_result = match auth {
            AuthMethod::Password(pw) => handle.authenticate_password(username, pw).await?,
            AuthMethod::KeyFile(path) => {
                let key_data = std::fs::read_to_string(path)?;
                let key_pair = russh::keys::PrivateKey::from_openssh(&key_data)
                    .map_err(|e| format!("Invalid key: {}", e))?;
                handle
                    .authenticate_publickey(
                        username,
                        PrivateKeyWithHashAlg::new(Arc::new(key_pair), None),
                    )
                    .await?
            }
            AuthMethod::Agent => {
                let mut agent = russh::keys::agent::client::AgentClient::connect_env()
                    .await
                    .map_err(|e| format!("Failed to connect to SSH agent: {}", e))?;
                let identities = agent
                    .request_identities()
                    .await
                    .map_err(|e| format!("Failed to list agent keys: {}", e))?;
                for identity in &identities {
                    let pubkey = match identity {
                        russh::keys::agent::AgentIdentity::PublicKey { key, .. } => key.clone(),
                        russh::keys::agent::AgentIdentity::Certificate { .. } => continue,
                    };
                    if let Ok(result) = handle
                        .authenticate_publickey_with(username, pubkey, None, &mut agent)
                        .await
                        && result.success()
                    {
                        return Ok(client::AuthResult::Success);
                    }
                }
                return Err("SSH agent: no accepted key found".into());
            }
        };
        Ok(auth_result)
    }

    pub async fn list_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, Box<dyn std::error::Error + Send + Sync>> {
        let read_dir = self.sftp.read_dir(path).await?;
        let mut entries = Vec::new();
        for entry in read_dir {
            let meta = entry.metadata();
            entries.push(RemoteEntry {
                name: entry.file_name(),
                is_dir: meta.file_type().is_dir(),
                size: meta.size.unwrap_or(0),
                mtime: meta.mtime.unwrap_or(0) as u64,
                permissions: meta.permissions.map(|p| p as u32).unwrap_or(0),
            });
        }
        entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.cmp(&b.name)));
        Ok(entries)
    }

    pub async fn download(
        &self,
        remote_path: &str,
        local_path: &str,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let meta = self.sftp.metadata(remote_path).await?;
        let total_size = meta.size.unwrap_or(0);
        let mut remote_file = self.sftp.open(remote_path).await?;
        let mut local_file = tokio::fs::File::create(local_path).await?;
        let mut transferred = 0u64;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = remote_file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            local_file.write_all(&buf[..n]).await?;
            transferred += n as u64;
            let _ = progress_tx.send(TransferProgress {
                transferred,
                total_size,
                done: false,
            });
        }
        local_file.flush().await?;
        let _ = progress_tx.send(TransferProgress {
            transferred,
            total_size,
            done: true,
        });
        Ok(())
    }

    pub async fn upload(
        &self,
        local_path: &str,
        remote_path: &str,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let local_meta = tokio::fs::metadata(local_path).await?;
        let total_size = local_meta.len();
        let mut local_file = tokio::fs::File::open(local_path).await?;
        let mut remote_file = self
            .sftp
            .open_with_flags(remote_path, OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE)
            .await?;
        let mut transferred = 0u64;
        let mut buf = vec![0u8; 256 * 1024];
        loop {
            let n = local_file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            remote_file.write_all(&buf[..n]).await?;
            transferred += n as u64;
            let _ = progress_tx.send(TransferProgress {
                transferred,
                total_size,
                done: false,
            });
        }
        remote_file.shutdown().await?;
        let _ = progress_tx.send(TransferProgress {
            transferred,
            total_size,
            done: true,
        });
        Ok(())
    }

    pub async fn mkdir(&self, path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.sftp.create_dir(path).await?;
        Ok(())
    }

    pub async fn remove_file(&self, path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.sftp.remove_file(path).await?;
        Ok(())
    }

    pub async fn remove_dir(&self, path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.sftp.remove_dir(path).await?;
        Ok(())
    }

    pub fn remove_dir_all<'a>(
        &'a self,
        path: &'a str,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
        Box::pin(async move {
            let entries = self.list_dir(path).await?;
            for entry in &entries {
                if entry.name == "." || entry.name == ".." {
                    continue;
                }
                let child = format!("{}/{}", path, entry.name);
                if entry.is_dir {
                    self.remove_dir_all(&child).await?;
                } else {
                    self.remove_file(&child).await?;
                }
            }
            self.remove_dir(path).await?;
            Ok(())
        })
    }

    pub async fn rename(&self, from: &str, to: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.sftp.rename(from, to).await?;
        Ok(())
    }

    pub async fn set_permissions(&self, path: &str, mode: u32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use russh_sftp::protocol::FileAttributes;
        let attrs = FileAttributes {
            permissions: Some(mode),
            ..Default::default()
        };
        self.sftp.set_metadata(path, attrs).await?;
        Ok(())
    }

    pub fn download_dir<'a>(
        &'a self,
        remote_path: &'a str,
        local_path: &'a str,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64, Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
        Box::pin(async move {
            tokio::fs::create_dir_all(local_path).await?;
            let entries = self.list_dir(remote_path).await?;
            let mut total_transferred = 0u64;
            for entry in &entries {
                if entry.name == "." || entry.name == ".." {
                    continue;
                }
                let remote = format!("{}/{}", remote_path, entry.name);
                let local = format!("{}/{}", local_path, entry.name);
                if entry.is_dir {
                    total_transferred += self.download_dir(&remote, &local, progress_tx.clone()).await?;
                } else {
                    self.download(&remote, &local, progress_tx.clone()).await?;
                    total_transferred += entry.size;
                }
            }
            Ok(total_transferred)
        })
    }

    pub fn upload_dir<'a>(
        &'a self,
        local_path: &'a str,
        remote_path: &'a str,
        progress_tx: mpsc::UnboundedSender<TransferProgress>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64, Box<dyn std::error::Error + Send + Sync>>> + Send + 'a>> {
        Box::pin(async move {
            let _ = self.mkdir(remote_path).await;
            let mut read_dir = tokio::fs::read_dir(local_path).await?;
            let mut total_transferred = 0u64;
            while let Some(entry) = read_dir.next_entry().await? {
                let name = entry.file_name().to_string_lossy().to_string();
                let local = format!("{}/{}", local_path, name);
                let remote = format!("{}/{}", remote_path, name);
                let meta = entry.metadata().await?;
                if meta.is_dir() {
                    total_transferred += self.upload_dir(&local, &remote, progress_tx.clone()).await?;
                } else {
                    self.upload(&local, &remote, progress_tx.clone()).await?;
                    total_transferred += meta.len();
                }
            }
            Ok(total_transferred)
        })
    }

    pub async fn exec_command(&self, command: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let channel = self.handle.channel_open_session().await?;
        channel.exec(true, command).await?;
        let mut output = Vec::new();
        let mut channel_stream = channel.into_stream();
        let mut buf = [0u8; 8192];
        loop {
            match tokio::time::timeout(std::time::Duration::from_secs(10), channel_stream.read(&mut buf)).await {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => output.extend_from_slice(&buf[..n]),
                Ok(Err(_)) => break,
            }
        }
        Ok(String::from_utf8_lossy(&output).to_string())
    }

    pub async fn close(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.sftp.close().await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct RemoteEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub mtime: u64,
    pub permissions: u32,
}

#[derive(Debug, Clone)]
pub struct TransferProgress {
    pub transferred: u64,
    pub total_size: u64,
    #[allow(dead_code)]
    pub done: bool,
}
