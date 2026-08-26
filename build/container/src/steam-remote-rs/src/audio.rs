use anyhow::{anyhow, Result};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;
use tokio::process::Child;

use crate::paths::{log_dir, AUDIO_SINK_DESCRIPTION, AUDIO_SINK_NAME};
use crate::process::{run_as_user, spawn_as_user, terminate};

pub struct AudioStack {
    pub pipewire: Option<Child>,
    pub wireplumber: Option<Child>,
    pub pulse: Option<Child>,
}

impl AudioStack {
    pub async fn start(runtime_dir: &Path, env: &BTreeMap<String, String>) -> Result<Self> {
        let mut pipewire = Some(spawn_as_user(&["pipewire"], env, "pipewire.log")?);
        if !wait_for_path(&runtime_dir.join("pipewire-0"), pipewire.as_mut()).await {
            terminate(&mut pipewire).await;
            return Err(anyhow!(
                "PipeWire did not create pipewire-0; see {}",
                log_dir().join("pipewire.log").display()
            ));
        }
        let wireplumber = Some(spawn_as_user(&["wireplumber"], env, "wireplumber.log")?);
        let mut pulse = Some(spawn_as_user(
            &["pipewire-pulse"],
            env,
            "pipewire-pulse.log",
        )?);
        if !wait_for_path(&runtime_dir.join("pulse/native"), pulse.as_mut()).await {
            let mut wireplumber = wireplumber;
            terminate(&mut pulse).await;
            terminate(&mut wireplumber).await;
            terminate(&mut pipewire).await;
            return Err(anyhow!(
                "PipeWire-Pulse did not create its native socket; see {}",
                log_dir().join("pipewire-pulse.log").display()
            ));
        }
        create_fallback_sink(env)?;
        Ok(Self {
            pipewire,
            wireplumber,
            pulse,
        })
    }

    pub async fn stop(&mut self) {
        terminate(&mut self.pulse).await;
        terminate(&mut self.wireplumber).await;
        terminate(&mut self.pipewire).await;
    }
}

async fn wait_for_path(path: &Path, child: Option<&mut Child>) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut child = child;
    while std::time::Instant::now() < deadline {
        if path.exists() {
            return true;
        }
        if child
            .as_deref_mut()
            .and_then(|child| child.try_wait().ok().flatten())
            .is_some()
        {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    false
}

fn create_fallback_sink(env: &BTreeMap<String, String>) -> Result<()> {
    let sinks = run_as_user(&["pactl", "list", "short", "sinks"], env)?;
    if !String::from_utf8_lossy(&sinks.stdout).contains(AUDIO_SINK_NAME) {
        let name = format!("sink_name={AUDIO_SINK_NAME}");
        let description = format!("sink_properties=device.description={AUDIO_SINK_DESCRIPTION}");
        let output = run_as_user(
            &[
                "pactl",
                "load-module",
                "module-null-sink",
                &name,
                "rate=48000",
                "channels=2",
                "channel_map=front-left,front-right",
                &description,
            ],
            env,
        )?;
        if !output.status.success() {
            return Err(anyhow!("failed to create the fallback PipeWire audio sink"));
        }
    }
    let _ = run_as_user(&["pactl", "set-default-sink", AUDIO_SINK_NAME], env);
    Ok(())
}
