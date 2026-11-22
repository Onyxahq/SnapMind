//! 录屏编码（M9.2）：MP4 用系统自带的 Media Foundation（H.264 + AAC，有显卡编码器时自动用上），
//! 系统声音用 WASAPI loopback（录下扬声器里正在放的声音）。都不需要附带 ffmpeg。

use std::path::Path;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    #[error("录屏编码失败：{0}")]
    Os(String),
    #[error("当前平台暂不支持录屏")]
    Unsupported,
}

/// 100 纳秒为单位的时间（Media Foundation 的时间单位）。
fn hns(d: Duration) -> i64 {
    (d.as_nanos() / 100) as i64
}

/// AAC 编码器只接受 44.1 / 48 kHz：其他采样率先重采样到 48 kHz。
pub fn aac_rate(device_rate: u32) -> u32 {
    if device_rate == 44_100 || device_rate == 48_000 { device_rate } else { 48_000 }
}

/// 把交错的多声道浮点采样转成 16 位立体声（多于两个声道只取前两个，单声道复制一份），
/// 采样率不同时线性插值。`pos` 是重采样的相位，跨批次保持连续。
pub fn to_stereo_i16(src: &[f32], channels: usize, from: u32, to: u32, pos: &mut f64, out: &mut Vec<i16>) {
    if channels == 0 || src.is_empty() {
        return;
    }
    let frames = src.len() / channels;
    let pick = |f: usize, c: usize| -> f32 {
        let c = if channels == 1 { 0 } else { c.min(channels - 1) };
        src[f * channels + c]
    };
    let conv = |v: f32| -> i16 { (v.clamp(-1.0, 1.0) * 32767.0).round() as i16 };
    if from == to {
        for f in 0..frames {
            out.push(conv(pick(f, 0)));
            out.push(conv(pick(f, 1)));
        }
        return;
    }
    let step = from as f64 / to as f64;
    while (*pos as usize) + 1 < frames {
        let i = *pos as usize;
        let t = (*pos - i as f64) as f32;
        for c in 0..2 {
            let v = pick(i, c) * (1.0 - t) + pick(i + 1, c) * t;
            out.push(conv(v));
        }
        *pos += step;
    }
    *pos -= frames as f64 - 1.0;
    if *pos < 0.0 {
        *pos = 0.0;
    }
}

pub use imp::{Loopback, Mp4Writer};

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::{GUID, HSTRING};
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator, MMDeviceEnumerator,
        AUDCLNT_BUFFERFLAGS_SILENT, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX,
        WAVEFORMATEXTENSIBLE,
    };
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED};

    fn os(e: impl std::fmt::Display) -> RecordError {
        RecordError::Os(e.to_string())
    }

    /// MF_VERSION（MF_SDK_VERSION << 16 | MF_API_VERSION）。
    const MF_VER: u32 = 0x0002_0070;

    /// 往 MP4 里写画面和声音。Media Foundation 的对象是自由线程的，可以在采集线程和声音线程之间共用
    /// （调用方用锁保证同一时间只有一个线程在写）。
    pub struct Mp4Writer {
        writer: IMFSinkWriter,
        video: u32,
        audio: Option<(u32, u32)>, // (流序号, 采样率)
        width: u32,
        height: u32,
        buf: Vec<u8>,
    }

    // SAFETY: IMFSinkWriter 运行在 MTA，自由线程；调用方用 Mutex 串行化所有调用。
    unsafe impl Send for Mp4Writer {}

    impl Mp4Writer {
        /// `audio_rate`：要写声音时给出采样率（44100 / 48000），立体声 16 位。
        pub fn create(path: &Path, width: u32, height: u32, fps: u32, audio_rate: Option<u32>) -> Result<Self, RecordError> {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                MFStartup(MF_VER, MFSTARTUP_FULL).map_err(os)?;
                let mut attrs: Option<IMFAttributes> = None;
                MFCreateAttributes(&mut attrs, 2).map_err(os)?;
                let attrs = attrs.ok_or_else(|| os("MFCreateAttributes"))?;
                attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, 1).map_err(os)?;
                let url = HSTRING::from(path.as_os_str());
                let writer = MFCreateSinkWriterFromURL(&url, None, &attrs).map_err(os)?;

                let size = ((width as u64) << 32) | height as u64;
                let rate = ((fps as u64) << 32) | 1;
                // 屏幕内容细节多：按 0.12 bit / 像素 / 帧估码率，限制在 2–40 Mbps
                let bitrate = ((width as f64 * height as f64 * fps as f64 * 0.12) as u32).clamp(2_000_000, 40_000_000);

                let out = MFCreateMediaType().map_err(os)?;
                out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(os)?;
                out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264).map_err(os)?;
                out.SetUINT32(&MF_MT_AVG_BITRATE, bitrate).map_err(os)?;
                out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).map_err(os)?;
                out.SetUINT32(&MF_MT_MPEG2_PROFILE, 77).map_err(os)?; // Main
                out.SetUINT64(&MF_MT_FRAME_SIZE, size).map_err(os)?;
                out.SetUINT64(&MF_MT_FRAME_RATE, rate).map_err(os)?;
                out.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1).map_err(os)?;
                let video = writer.AddStream(&out).map_err(os)?;

                let input = MFCreateMediaType().map_err(os)?;
                input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video).map_err(os)?;
                input.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32).map_err(os)?;
                input.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32).map_err(os)?;
                input.SetUINT64(&MF_MT_FRAME_SIZE, size).map_err(os)?;
                input.SetUINT64(&MF_MT_FRAME_RATE, rate).map_err(os)?;
                input.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1).map_err(os)?;
                writer.SetInputMediaType(video, &input, None).map_err(os)?;

                let audio = match audio_rate {
                    Some(r) => {
                        let out = MFCreateMediaType().map_err(os)?;
                        out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(os)?;
                        out.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC).map_err(os)?;
                        out.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(os)?;
                        out.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, r).map_err(os)?;
                        out.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2).map_err(os)?;
                        out.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 24_000).map_err(os)?; // 192 kbps
                        let idx = writer.AddStream(&out).map_err(os)?;
                        let input = MFCreateMediaType().map_err(os)?;
                        input.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio).map_err(os)?;
                        input.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM).map_err(os)?;
                        input.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16).map_err(os)?;
                        input.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, r).map_err(os)?;
                        input.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2).map_err(os)?;
                        input.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4).map_err(os)?;
                        input.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, r * 4).map_err(os)?;
                        writer.SetInputMediaType(idx, &input, None).map_err(os)?;
                        Some((idx, r))
                    }
                    None => None,
                };
                writer.BeginWriting().map_err(os)?;
                Ok(Self { writer, video, audio, width, height, buf: Vec::new() })
            }
        }

        fn write(&self, stream: u32, bytes: &[u8], time: i64, dur: i64) -> Result<(), RecordError> {
            unsafe {
                let buffer = MFCreateMemoryBuffer(bytes.len() as u32).map_err(os)?;
                let mut ptr: *mut u8 = std::ptr::null_mut();
                buffer.Lock(&mut ptr, None, None).map_err(os)?;
                std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
                buffer.Unlock().map_err(os)?;
                buffer.SetCurrentLength(bytes.len() as u32).map_err(os)?;
                let sample = MFCreateSample().map_err(os)?;
                sample.AddBuffer(&buffer).map_err(os)?;
                sample.SetSampleTime(time).map_err(os)?;
                sample.SetSampleDuration(dur).map_err(os)?;
                self.writer.WriteSample(stream, &sample).map_err(os)
            }
        }

        /// 写一帧画面（BGRA，自上而下，宽高与创建时一致）。`t` 是这一帧在视频里的时间。
        pub fn video(&mut self, bgra: &[u8], t: Duration, dur: Duration) -> Result<(), RecordError> {
            // RGB32 默认自下而上：逐行倒过来拷
            let stride = self.width as usize * 4;
            let h = self.height as usize;
            self.buf.resize(stride * h, 0);
            for y in 0..h {
                let src = &bgra[y * stride..(y + 1) * stride];
                self.buf[(h - 1 - y) * stride..(h - y) * stride].copy_from_slice(src);
            }
            let buf = std::mem::take(&mut self.buf);
            let r = self.write(self.video, &buf, hns(t), hns(dur));
            self.buf = buf;
            r
        }

        /// 写一段声音（16 位立体声交错采样）。`t` 是这段声音开头在视频里的时间。
        pub fn audio(&mut self, pcm: &[i16], t: Duration) -> Result<(), RecordError> {
            let Some((stream, rate)) = self.audio else { return Ok(()) };
            if pcm.is_empty() {
                return Ok(());
            }
            let frames = pcm.len() / 2;
            let dur = Duration::from_secs_f64(frames as f64 / rate as f64);
            let bytes = unsafe { std::slice::from_raw_parts(pcm.as_ptr() as *const u8, pcm.len() * 2) };
            self.write(stream, bytes, hns(t), hns(dur))
        }

        /// 写完：补上文件尾。
        pub fn finish(self) -> Result<(), RecordError> {
            let r = unsafe { self.writer.Finalize() }.map_err(os);
            drop(self.writer);
            unsafe {
                let _ = MFShutdown();
            }
            r
        }
    }

    const SUBTYPE_FLOAT: GUID = GUID::from_u128(0x00000003_0000_0010_8000_00aa00389b71);
    const SUBTYPE_PCM: GUID = GUID::from_u128(0x00000001_0000_0010_8000_00aa00389b71);
    const TAG_PCM: u16 = 1;
    const TAG_FLOAT: u16 = 3;
    const TAG_EXTENSIBLE: u16 = 0xFFFE;

    /// 系统声音（默认播放设备的 loopback）。在哪个线程打开就在哪个线程读。
    pub struct Loopback {
        client: IAudioClient,
        capture: IAudioCaptureClient,
        rate: u32,
        channels: usize,
        float: bool,
        bits: u16,
        scratch: Vec<f32>,
    }

    // SAFETY: 在 MTA 里创建的 WASAPI 对象是自由线程的；打开后只交给一条线程使用。
    unsafe impl Send for Loopback {}

    impl Loopback {
        pub fn open() -> Result<Self, RecordError> {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(os)?;
                let device = en.GetDefaultAudioEndpoint(eRender, eConsole).map_err(os)?;
                let client: IAudioClient = device.Activate(CLSCTX_ALL, None).map_err(os)?;
                let fmt = client.GetMixFormat().map_err(os)?;
                let wf: WAVEFORMATEX = *fmt;
                let (float, bits) = match wf.wFormatTag {
                    TAG_FLOAT => (true, wf.wBitsPerSample),
                    TAG_PCM => (false, wf.wBitsPerSample),
                    TAG_EXTENSIBLE => {
                        let ext: WAVEFORMATEXTENSIBLE = *(fmt as *const WAVEFORMATEXTENSIBLE);
                        let sub = ext.SubFormat;
                        if sub == SUBTYPE_FLOAT {
                            (true, wf.wBitsPerSample)
                        } else if sub == SUBTYPE_PCM {
                            (false, wf.wBitsPerSample)
                        } else {
                            CoTaskMemFree(Some(fmt as *const _));
                            return Err(os("不支持的声音格式"));
                        }
                    }
                    _ => {
                        CoTaskMemFree(Some(fmt as *const _));
                        return Err(os("不支持的声音格式"));
                    }
                };
                let init = client.Initialize(AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, 10_000_000, 0, fmt, None);
                CoTaskMemFree(Some(fmt as *const _));
                init.map_err(os)?;
                let capture: IAudioCaptureClient = client.GetService().map_err(os)?;
                client.Start().map_err(os)?;
                Ok(Self {
                    client,
                    capture,
                    rate: wf.nSamplesPerSec,
                    channels: wf.nChannels as usize,
                    float,
                    bits,
                    scratch: Vec::new(),
                })
            }
        }

        pub fn rate(&self) -> u32 {
            self.rate
        }

        pub fn channels(&self) -> usize {
            self.channels
        }

        /// 读出现在已经到的所有采样（交错浮点，设备的声道数和采样率）。没有声音在放时可能什么都没有。
        pub fn read(&mut self) -> Result<&[f32], RecordError> {
            self.scratch.clear();
            unsafe {
                loop {
                    let n = self.capture.GetNextPacketSize().map_err(os)?;
                    if n == 0 {
                        break;
                    }
                    let mut data: *mut u8 = std::ptr::null_mut();
                    let mut frames = 0u32;
                    let mut flags = 0u32;
                    self.capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None).map_err(os)?;
                    let count = frames as usize * self.channels;
                    if flags & AUDCLNT_BUFFERFLAGS_SILENT.0 as u32 != 0 || data.is_null() {
                        self.scratch.extend(std::iter::repeat_n(0.0, count));
                    } else if self.float && self.bits == 32 {
                        self.scratch.extend_from_slice(std::slice::from_raw_parts(data as *const f32, count));
                    } else if !self.float && self.bits == 16 {
                        let s = std::slice::from_raw_parts(data as *const i16, count);
                        self.scratch.extend(s.iter().map(|&v| v as f32 / 32768.0));
                    } else if !self.float && self.bits == 32 {
                        let s = std::slice::from_raw_parts(data as *const i32, count);
                        self.scratch.extend(s.iter().map(|&v| v as f32 / 2_147_483_648.0));
                    } else {
                        self.scratch.extend(std::iter::repeat_n(0.0, count));
                    }
                    self.capture.ReleaseBuffer(frames).map_err(os)?;
                }
            }
            Ok(&self.scratch)
        }
    }

    impl Drop for Loopback {
        fn drop(&mut self) {
            unsafe {
                let _ = self.client.Stop();
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub struct Mp4Writer;
    impl Mp4Writer {
        pub fn create(_: &Path, _: u32, _: u32, _: u32, _: Option<u32>) -> Result<Self, RecordError> {
            Err(RecordError::Unsupported)
        }
        pub fn video(&mut self, _: &[u8], _: Duration, _: Duration) -> Result<(), RecordError> {
            Err(RecordError::Unsupported)
        }
        pub fn audio(&mut self, _: &[i16], _: Duration) -> Result<(), RecordError> {
            Err(RecordError::Unsupported)
        }
        pub fn finish(self) -> Result<(), RecordError> {
            Err(RecordError::Unsupported)
        }
    }

    pub struct Loopback;
    impl Loopback {
        pub fn open() -> Result<Self, RecordError> {
            Err(RecordError::Unsupported)
        }
        pub fn rate(&self) -> u32 {
            48_000
        }
        pub fn channels(&self) -> usize {
            2
        }
        pub fn read(&mut self) -> Result<&[f32], RecordError> {
            Err(RecordError::Unsupported)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stereo_passthrough_and_mono_duplicate() {
        let mut out = Vec::new();
        let mut pos = 0.0;
        to_stereo_i16(&[0.5, -0.5, 1.0, 0.0], 2, 48_000, 48_000, &mut pos, &mut out);
        assert_eq!(out, vec![16384, -16384, 32767, 0]);
        out.clear();
        to_stereo_i16(&[0.25, 2.0], 1, 48_000, 48_000, &mut pos, &mut out);
        assert_eq!(out, vec![8192, 8192, 32767, 32767]);
    }

    #[test]
    fn surround_keeps_front_pair() {
        let mut out = Vec::new();
        let mut pos = 0.0;
        // 6 声道：只取前两个
        to_stereo_i16(&[0.1, 0.2, 0.9, 0.9, 0.9, 0.9], 6, 48_000, 48_000, &mut pos, &mut out);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], (0.1f32 * 32767.0).round() as i16);
    }

    #[test]
    fn resample_keeps_length_ratio_across_batches() {
        // 96 kHz → 48 kHz：分三批喂，总输出约为一半，相位连续
        let mut out = Vec::new();
        let mut pos = 0.0;
        let batch: Vec<f32> = (0..2000).map(|i| (i % 100) as f32 / 100.0).collect(); // 1000 帧立体声
        for _ in 0..3 {
            to_stereo_i16(&batch, 2, 96_000, 48_000, &mut pos, &mut out);
        }
        let frames = out.len() / 2;
        assert!((1495..=1501).contains(&frames), "{frames}");
        assert_eq!(aac_rate(96_000), 48_000);
        assert_eq!(aac_rate(44_100), 44_100);
    }

    /// 真的用 Media Foundation 写一个 1 秒的小 MP4（带静音音轨），检查文件头。
    #[cfg(windows)]
    #[test]
    fn writes_a_small_mp4() {
        let dir = std::env::temp_dir().join(format!("snapmind-mp4-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.mp4");
        let (w, h) = (320u32, 240u32);
        let mut m = Mp4Writer::create(&path, w, h, 10, Some(48_000)).expect("创建");
        for i in 0..10u32 {
            let px: Vec<u8> = (0..w * h).flat_map(|p| [(p % 256) as u8, (i * 20) as u8, 0, 255]).collect();
            m.video(&px, Duration::from_millis(i as u64 * 100), Duration::from_millis(100)).expect("画面");
            m.audio(&vec![0i16; 4800 * 2], Duration::from_millis(i as u64 * 100)).expect("声音");
        }
        m.finish().expect("写完");
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.len() > 1000, "{}", bytes.len());
        assert_eq!(&bytes[4..8], b"ftyp");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
