// Copyright 2026 studyreadbook4ever
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fs;
// mpv의 Graceful Shutdown을 위해 std->tokio 수정
use tokio::process::Command;

#[allow(non_snake_case)]
#[allow(dead_code)]

#[derive(Deserialize)]
struct Config {
    #[serde(flatten)]
    channels: HashMap<String, String>,
}

#[derive(Deserialize)]
struct Wrapper {
    channels: HashMap<String, String>,
}
#[derive(Deserialize)]
struct ChzzkContent {
    livePlaybackJson: Option<String>,
}
#[derive(Deserialize)]
struct ChzzkResponse {
    code: i32,
    content: Option<ChzzkContent>,
}
#[derive(Deserialize)]
struct PlaybackData {
    media: Vec<MediaInfo>,
}
#[derive(Deserialize)]
struct MediaInfo {
    protocol: String,
    path: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: Program <channel name on channels.toml> <quality(1080/720/480/360/144),default:1080>");
        return Ok(());
    }
    let alias = &args[1];
    let quality = args.get(2).map(|s| s.as_str()).unwrap_or("1080");
    let bitrate_limit = match quality {
        "1080" => "0",       //제한 x, 1080p
        "720" => "5000000",  //720p
        "480" => "2500000",  //480p
        "360" => "1200000",  //360p
        "144" => "500000",   //144p
        "0" => "500000",     //144p, 어차피 버릴거임.
        _ => "0",
    };
    
    if quality == "0" {
        println!("it will be radio mode...");
    } else if bitrate_limit != "0" {
        println!("quality limit on... {}p will be stream", quality);
    } else {
        println!("quality has no limit... it will stream 1080p");
    }

    let toml_str = fs::read_to_string("channels.toml").map_err(|_| "channels.toml file cannot be readed...")?;
    let channels = if let Ok(wrapper) = toml::from_str::<Wrapper>(&toml_str) {
        wrapper.channels
    } else {
        let err_msg = format!(
            "channels.toml structure is wrong... check it again...\n\n\
            right usage: \"랄로\" = \"3497a9a7221cc3ee5d3f95991d9f95e9\""
        );
        toml::from_str::<HashMap<String, String>>(&toml_str).map_err(|_| err_msg)?
    };
    
    let channel_id = match channels.get(alias) {
        Some(id) => id,
        None => {
            eprintln!("We cannont find {} channel...", alias);
            return Ok(());
        }
    };

    println!("{} channel connecting...", alias);

    let client = reqwest::Client::new();
    let url = format!("https://api.chzzk.naver.com/service/v2/channels/{}/live-detail", channel_id);
    let resp = client.get(&url).header("User-Agent", "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/121.0.0.0 Safari/537.36")
        .send().await?.json::<ChzzkResponse>().await?;

    let content = resp.content.ok_or("We cannot find streaming...")?;
    let playback_json = content.livePlaybackJson.ok_or("we are not streaming...")?;
    let playback_data: PlaybackData = serde_json::from_str(&playback_json)?;
    let hls_url = playback_data.media.iter().find(|m| m.protocol == "HLS").map(|m| &m.path).ok_or("no stream")?;
    println!("Stream get finish!");


    // 터미널에서 Ctrl+C를 누를 때 Rust가 즉사하는 것을 막고 mpv의 종료를 기다립니다.
    tokio::spawn(async {
        let _ = tokio::signal::ctrl_c().await;
        println!("\n[System] Ctrl+C 감지! mpv가 잔상을 지우고 안전하게 닫힐 때까지 기다려줍니다...");
    });
    // =========================================================================

    // 라디오 모드 
    if quality == "0" {
        println!("Starting on RADIO Mode...");
        let status = Command::new("mpv").arg(hls_url)
            .arg("--hls-bitrate=500000") // 명목상 144p로 세팅.
            .arg("--vid=no")
            .arg("--force-window=no")
            .arg("--audio-display=no")
            .arg("--ao=pulse,pipewire,alsa,auto")
            .arg("--volume=100")
            .arg("--audio-buffer=2.0")          // 오디오 버퍼 넉넉하게
            .arg("--cache=yes")
            .arg("--demuxer-max-bytes=10MiB")   // 메모리 절약
            .arg("--demuxer-readahead-secs=10") 
            .arg("--msg-level=ffmpeg=error,demuxer=error")
            .status()
            .await; // 비동기 대기 추가

        match status {
            Ok(_) => println!("stream finish!!"),
            Err(e) => eprintln!("MPV run failed...{}", e),
        }
        return Ok(());
    }

    // TV 모드
    println!("Starting on TV Mode...");
    let status = Command::new("mpv").arg(hls_url).args(if bitrate_limit != "0" { vec![format!("--hls-bitrate={}", bitrate_limit)] } else { vec![] })
        .arg("--ao=pulse,pipewire,alsa,auto")
        .arg("--volume=100")
        .arg("--audio-buffer=0.5") //너무 작아도 커도 이상. 작으면 starvation
        .arg("--audio-wait-open=0") //일단 화면부터
        .arg("--framedrop=vo")     //오디오 싱크 우선
        .arg("--video-sync=display-resample") //display-resample(싱크) vs audio(딜레이 잡기) 근데 A-V싱크가 압도적으로 중요하니..
        .arg("--autosync=30")
        .arg("--stream-buffer-size=1MiB")
        .arg("--cache=yes") 
        .arg("--demuxer-max-bytes=32MiB")
        .arg("--demuxer-max-back-bytes=0") // 되돌리는 기능 x
        .arg("--demuxer-readahead-secs=1") //미리 처리 아주약간
        .arg("--keepaspect-window=yes")   // 비율 유지
        .arg("--autofit=40%")             // 모니터 가로 픽셀의 40% 크기에 맞춤
        .arg("--geometry=-0+0")           // 우측 상단 끝 배치
        .arg("--window-maximized=no")
        .arg("--window-minimized=no")
        .arg("--no-keepaspect-window")    //창크기 조정 불가
        .arg("--no-border")
        .arg("--ontop")
        .arg("--no-window-dragging")
        .arg("--no-input-default-bindings")
        .arg("--input-vo-keyboard=no")
        .arg("--cursor-autohide=always")
        .arg("--no-osc")
        .arg("--no-input-cursor")
        .arg("--gpu-api=auto")
        .arg("--vd-lavc-dr=yes") // no memory-gpu copy
        .arg("--vo=gpu")
        .arg("--gpu-context=auto")
        .arg("--hwdec=auto-safe")
        .arg("--sws-allow-zimg=no")
        .arg("--scale=bilinear")
        .arg("--cscale=bilinear")
        .arg("--dither-depth=no") //계단현상 보정옵션 끄기
        .arg("--msg-level=ffmpeg=error,demuxer=error")
        .status()
        .await; // 비동기 대기 추가

    match status {
        Ok(_) => println!("stream finish!!"),
        Err(e) => eprintln!("MPV run failed...{}", e),
    }
    
    Ok(())
}
