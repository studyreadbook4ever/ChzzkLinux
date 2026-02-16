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
use std::process::Command;

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
    "1080" => "0",      //제한 x, 1080p
    "720" => "5000000",  //720p
    "480" => "2500000",  //480p
    "360" => "1200000", //360p
    "144" => "500000",   //144p
    //"0" => 라디오모드 추후 제작예정
    _ => "0",
  };

  if bitrate_limit != "0" {
    println!("quality limit on... {}p will be stream", quality);
  } else {
    println!("quality has no limit... it will stream 1080p");
  }

  let toml_str = fs::read_to_string("channels.toml").map_err(|_| "channels.toml file cannot be readed...")?;
  let channels = if let Ok(wrapper) = toml::from_str::<Wrapper>(&toml_str) {
    wrapper.channels
  } else {
    // 에러 발생 시 출력할 메시지를 깔끔하게 구성
    let err_msg = format!(
        "channels.toml structure is wrong... check it again...\n\n\
        right usage: \"랄로\" = \"3497a9a7221cc3ee5d3f95991d9f95e9\""
    );

    // map_err로 메시지를 전달하고, ?로 즉시 에러 반환
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
  println!("Starting on TV Mode...");

  //starting by mpv library
  //라디오모드 추가해야...
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
    //.arg("--demuxer-thread=no") low buffer usage

    //화면 배치
    .arg("--keepaspect-window=yes")   // 비율 유지
    .arg("--autofit=30%")             // 모니터 가로 픽셀의 30% 크기에 맞춤
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
    .arg("--cursor-autohide=always")

    //코덱 최적화
    .arg("--gpu-api=auto") //auto-safe보다 나음. gpu는 애초에 화면송출에 병목이 어지간해서 없음..
    .arg("--vd-lavc-dr=yes") // no memory-gpu copy
    .arg("--vo=gpu")
    .arg("--gpu-context=auto")
    //.arg("--vd-lavc-threads=??") 쓰레드를 얼마로 해줘야하려나..
    .arg("--hwdec=auto-safe")

    //.arg("--profile=fast")     //화면 크기맞춤, 각종 고 오버헤드 옵션 전부 비활성화 .. 근데 생각보다 얻는옵션이 적음
    .arg("--sws-allow-zimg=no")
    .arg("--scale=bilinear")
    .arg("--cscale=bilinear")
    .arg("--dither-depth=no") //계단현상 보정옵션 끄기
    //.arg("--correct-downscaling=no") //밝기 보정 옵션 끄기
    //.arg("--linear-downscaling=no") //선형축소 x
    //.arg("--sigmoid-upscaling=no") //확대 x
    .arg("--msg-level=ffmpeg=error,demuxer=error")
    .status();

  match status {
    Ok(_) => println!("stream finish!!"),
    Err(e) => eprintln!("MPV run failed...{}", e),
  }
  Ok(())
}

