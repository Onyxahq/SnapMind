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

/// macOS：MP4 用 AVAssetWriter（系统自带的 H.264 / AAC 编码器，有硬件编码时自动用上）。
/// 系统声音（`Loopback`）要靠 ScreenCaptureKit 的音频流，还没做（MAC-3.1 第三步），先报不支持——
/// 上层会退回只录画面。
#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::AnyThread;
    use objc2_av_foundation::{
        AVAssetWriter, AVAssetWriterInput, AVAssetWriterInputPixelBufferAdaptor, AVAssetWriterStatus, AVFileTypeMPEG4,
        AVMediaTypeAudio, AVMediaTypeVideo, AVVideoAverageBitRateKey, AVVideoCodecKey, AVVideoCodecTypeH264,
        AVVideoCompressionPropertiesKey, AVVideoExpectedSourceFrameRateKey, AVVideoHeightKey, AVVideoProfileLevelH264MainAutoLevel,
        AVVideoProfileLevelKey, AVVideoWidthKey,
    };
    use objc2_core_media::{CMSampleBuffer, CMTime, CMTimeFlags};
    use objc2_core_video::CVPixelBuffer;
    use objc2_foundation::{NSCopying, NSDictionary, NSNumber, NSString, NSURL};
    use std::ffi::c_void;

    fn os(e: impl std::fmt::Display) -> RecordError {
        RecordError::Os(e.to_string())
    }

    #[link(name = "CoreVideo", kind = "framework")]
    unsafe extern "C" {
        fn CVPixelBufferPoolCreatePixelBuffer(alloc: *const c_void, pool: *const c_void, out: *mut *mut c_void) -> i32;
        fn CVPixelBufferLockBaseAddress(buf: *mut c_void, flags: u64) -> i32;
        fn CVPixelBufferUnlockBaseAddress(buf: *mut c_void, flags: u64) -> i32;
        fn CVPixelBufferGetBaseAddress(buf: *mut c_void) -> *mut u8;
        fn CVPixelBufferGetBytesPerRow(buf: *mut c_void) -> usize;
        fn CVPixelBufferRelease(buf: *mut c_void);
    }

    /// AudioStreamBasicDescription
    #[repr(C)]
    struct Asbd {
        sample_rate: f64,
        format_id: u32,
        format_flags: u32,
        bytes_per_packet: u32,
        frames_per_packet: u32,
        bytes_per_frame: u32,
        channels_per_frame: u32,
        bits_per_channel: u32,
        reserved: u32,
    }

    #[link(name = "CoreMedia", kind = "framework")]
    unsafe extern "C" {
        fn CMAudioFormatDescriptionCreate(
            alloc: *const c_void,
            asbd: *const Asbd,
            layout_size: usize,
            layout: *const c_void,
            cookie_size: usize,
            cookie: *const c_void,
            extensions: *const c_void,
            out: *mut *const c_void,
        ) -> i32;
        fn CMBlockBufferCreateWithMemoryBlock(
            alloc: *const c_void,
            memory: *mut c_void,
            block_len: usize,
            block_alloc: *const c_void,
            source: *const c_void,
            offset: usize,
            data_len: usize,
            flags: u32,
            out: *mut *mut c_void,
        ) -> i32;
        fn CMBlockBufferReplaceDataBytes(src: *const c_void, dest: *mut c_void, offset: usize, len: usize) -> i32;
        fn CMAudioSampleBufferCreateReadyWithPacketDescriptions(
            alloc: *const c_void,
            data: *mut c_void,
            format: *const c_void,
            frames: isize,
            pts: CMTime,
            packets: *const c_void,
            out: *mut *mut c_void,
        ) -> i32;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
    }

    const BGRA: u32 = u32::from_be_bytes(*b"BGRA");
    const AAC: u32 = u32::from_be_bytes(*b"aac ");
    const LPCM: u32 = u32::from_be_bytes(*b"lpcm");
    /// 有符号整数 | 紧凑排列
    const PCM_FLAGS: u32 = 4 | 8;

    fn time(t: Duration) -> CMTime {
        CMTime { value: t.as_micros() as i64, timescale: 1_000_000, flags: CMTimeFlags::Valid, epoch: 0 }
    }

    fn dict(pairs: &[(&NSString, Retained<AnyObject>)]) -> Retained<NSDictionary<NSString, AnyObject>> {
        let keys: Vec<&NSString> = pairs.iter().map(|(k, _)| *k).collect();
        let values: Vec<&AnyObject> = pairs.iter().map(|(_, v)| &**v).collect();
        NSDictionary::from_slices(&keys, &values)
    }

    fn num(v: f64) -> Retained<AnyObject> {
        Retained::into_super(Retained::into_super(NSNumber::new_f64(v))).into()
    }

    fn string(v: &NSString) -> Retained<AnyObject> {
        Retained::into_super(Retained::into_super(v.copy())).into()
    }

    fn key(k: Option<&'static NSString>) -> Result<&'static NSString, RecordError> {
        k.ok_or_else(|| os("系统缺少 AVFoundation 常量"))
    }

    pub struct Mp4Writer {
        writer: Retained<AVAssetWriter>,
        video: Retained<AVAssetWriterInput>,
        adaptor: Retained<AVAssetWriterInputPixelBufferAdaptor>,
        audio: Option<(Retained<AVAssetWriterInput>, *const c_void, u32)>, // (输入, 声音格式描述, 采样率)
        width: u32,
        height: u32,
        last: Option<Duration>,
    }
    // SAFETY: AVAssetWriter 可以在任意线程使用；调用方用 Mutex 串行化所有调用。
    unsafe impl Send for Mp4Writer {}

    impl Mp4Writer {
        /// `audio_rate`：要写声音时给出采样率（44100 / 48000），立体声 16 位。
        pub fn create(path: &Path, width: u32, height: u32, fps: u32, audio_rate: Option<u32>) -> Result<Self, RecordError> {
            // AVAssetWriter 不覆盖已有文件
            let _ = std::fs::remove_file(path);
            let url = NSURL::from_file_path(path).ok_or_else(|| os("文件路径无效"))?;
            let writer = unsafe {
                AVAssetWriter::initWithURL_fileType_error(AVAssetWriter::alloc(), &url, key(AVFileTypeMPEG4)?)
            }
            .map_err(|e| os(e.localizedDescription()))?;

            // 与 Windows 相同：按 0.12 bit / 像素 / 帧估码率，限制在 2–40 Mbps
            let bitrate = (width as f64 * height as f64 * fps as f64 * 0.12).clamp(2_000_000.0, 40_000_000.0);
            let compression = dict(&[
                (key(unsafe { AVVideoAverageBitRateKey })?, num(bitrate)),
                (key(unsafe { AVVideoExpectedSourceFrameRateKey })?, num(fps as f64)),
                (
                    key(unsafe { AVVideoProfileLevelKey })?,
                    string(key(unsafe { AVVideoProfileLevelH264MainAutoLevel })?),
                ),
            ]);
            let settings = dict(&[
                (key(unsafe { AVVideoCodecKey })?, string(key(unsafe { AVVideoCodecTypeH264 })?)),
                (key(unsafe { AVVideoWidthKey })?, num(width as f64)),
                (key(unsafe { AVVideoHeightKey })?, num(height as f64)),
                (key(unsafe { AVVideoCompressionPropertiesKey })?, Retained::into_super(Retained::into_super(compression)).into()),
            ]);
            let video = unsafe {
                AVAssetWriterInput::initWithMediaType_outputSettings(
                    AVAssetWriterInput::alloc(),
                    key(AVMediaTypeVideo)?,
                    Some(&settings),
                )
            };
            unsafe { video.setExpectsMediaDataInRealTime(true) };
            let attrs = dict(&[
                (&NSString::from_str("PixelFormatType"), num(BGRA as f64)),
                (&NSString::from_str("Width"), num(width as f64)),
                (&NSString::from_str("Height"), num(height as f64)),
            ]);
            let adaptor = unsafe {
                AVAssetWriterInputPixelBufferAdaptor::initWithAssetWriterInput_sourcePixelBufferAttributes(
                    AVAssetWriterInputPixelBufferAdaptor::alloc(),
                    &video,
                    Some(&attrs),
                )
            };
            if !unsafe { writer.canAddInput(&video) } {
                return Err(os("不能添加视频轨"));
            }
            unsafe { writer.addInput(&video) };

            let audio = match audio_rate {
                Some(rate) => {
                    // 这几个键的值就是它们的名字（AVAudioSettings.h）
                    let settings = dict(&[
                        (&NSString::from_str("AVFormatIDKey"), num(AAC as f64)),
                        (&NSString::from_str("AVSampleRateKey"), num(rate as f64)),
                        (&NSString::from_str("AVNumberOfChannelsKey"), num(2.0)),
                        (&NSString::from_str("AVEncoderBitRateKey"), num(192_000.0)),
                    ]);
                    let input = unsafe {
                        AVAssetWriterInput::initWithMediaType_outputSettings(
                            AVAssetWriterInput::alloc(),
                            key(AVMediaTypeAudio)?,
                            Some(&settings),
                        )
                    };
                    unsafe { input.setExpectsMediaDataInRealTime(true) };
                    if !unsafe { writer.canAddInput(&input) } {
                        return Err(os("不能添加声音轨"));
                    }
                    unsafe { writer.addInput(&input) };
                    let asbd = Asbd {
                        sample_rate: rate as f64,
                        format_id: LPCM,
                        format_flags: PCM_FLAGS,
                        bytes_per_packet: 4,
                        frames_per_packet: 1,
                        bytes_per_frame: 4,
                        channels_per_frame: 2,
                        bits_per_channel: 16,
                        reserved: 0,
                    };
                    let mut fmt = std::ptr::null();
                    let st = unsafe {
                        CMAudioFormatDescriptionCreate(
                            std::ptr::null(),
                            &asbd,
                            0,
                            std::ptr::null(),
                            0,
                            std::ptr::null(),
                            std::ptr::null(),
                            &mut fmt,
                        )
                    };
                    if st != 0 {
                        return Err(os(format!("声音格式 {st}")));
                    }
                    Some((input, fmt, rate))
                }
                None => None,
            };

            if !unsafe { writer.startWriting() } {
                return Err(os(unsafe { writer.error() }.map(|e| e.localizedDescription().to_string()).unwrap_or_default()));
            }
            unsafe { writer.startSessionAtSourceTime(time(Duration::ZERO)) };
            Ok(Self { writer, video, adaptor, audio, width, height, last: None })
        }

        /// 编码器忙不过来时最多等一会儿，还不行就丢掉这一帧 / 这段（实时录制，宁丢不卡）。
        fn ready(input: &AVAssetWriterInput) -> bool {
            for _ in 0..25 {
                if unsafe { input.isReadyForMoreMediaData() } {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            false
        }

        fn failed(&self) -> RecordError {
            os(unsafe { self.writer.error() }.map(|e| e.localizedDescription().to_string()).unwrap_or_else(|| "写入失败".into()))
        }

        /// 写一帧画面（BGRA，自上而下，宽高与创建时一致）。`t` 是这一帧在视频里的时间。
        pub fn video(&mut self, bgra: &[u8], t: Duration, _dur: Duration) -> Result<(), RecordError> {
            // 时间必须严格递增
            if self.last.is_some_and(|l| t <= l) {
                return Ok(());
            }
            if !Self::ready(&self.video) {
                return Ok(());
            }
            let Some(pool) = (unsafe { self.adaptor.pixelBufferPool() }) else { return Err(self.failed()) };
            let mut buf: *mut c_void = std::ptr::null_mut();
            let st = unsafe { CVPixelBufferPoolCreatePixelBuffer(std::ptr::null(), &*pool as *const _ as *const c_void, &mut buf) };
            if st != 0 || buf.is_null() {
                return Err(os(format!("取像素缓冲 {st}")));
            }
            let row = self.width as usize * 4;
            unsafe {
                CVPixelBufferLockBaseAddress(buf, 0);
                let base = CVPixelBufferGetBaseAddress(buf);
                let stride = CVPixelBufferGetBytesPerRow(buf);
                for y in 0..self.height as usize {
                    std::ptr::copy_nonoverlapping(bgra[y * row..].as_ptr(), base.add(y * stride), row);
                }
                CVPixelBufferUnlockBaseAddress(buf, 0);
            }
            let ok = unsafe { self.adaptor.appendPixelBuffer_withPresentationTime(&*(buf as *const CVPixelBuffer), time(t)) };
            unsafe { CVPixelBufferRelease(buf) };
            if !ok {
                return Err(self.failed());
            }
            self.last = Some(t);
            Ok(())
        }

        /// 写一段声音（16 位立体声交错采样）。`t` 是这段声音开头在视频里的时间。
        pub fn audio(&mut self, pcm: &[i16], t: Duration) -> Result<(), RecordError> {
            let Some((input, fmt, _rate)) = &self.audio else { return Ok(()) };
            if pcm.is_empty() || !Self::ready(input) {
                return Ok(());
            }
            let bytes = pcm.len() * 2;
            unsafe {
                let mut block: *mut c_void = std::ptr::null_mut();
                // 1 = kCMBlockBufferAssureMemoryNowFlag：现在就分配内存
                let st = CMBlockBufferCreateWithMemoryBlock(
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    bytes,
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    bytes,
                    1,
                    &mut block,
                );
                if st != 0 {
                    return Err(os(format!("声音缓冲 {st}")));
                }
                CMBlockBufferReplaceDataBytes(pcm.as_ptr() as *const c_void, block, 0, bytes);
                let mut sample: *mut c_void = std::ptr::null_mut();
                let st = CMAudioSampleBufferCreateReadyWithPacketDescriptions(
                    std::ptr::null(),
                    block,
                    *fmt,
                    (pcm.len() / 2) as isize,
                    time(t),
                    std::ptr::null(),
                    &mut sample,
                );
                CFRelease(block);
                if st != 0 || sample.is_null() {
                    return Err(os(format!("声音样本 {st}")));
                }
                let ok = input.appendSampleBuffer(&*(sample as *const CMSampleBuffer));
                CFRelease(sample);
                if !ok {
                    return Err(self.failed());
                }
            }
            Ok(())
        }

        pub fn finish(self) -> Result<(), RecordError> {
            unsafe {
                self.video.markAsFinished();
                if let Some((input, _, _)) = &self.audio {
                    input.markAsFinished();
                }
                let (tx, rx) = std::sync::mpsc::channel();
                let block = block2::RcBlock::new(move || {
                    let _ = tx.send(());
                });
                self.writer.finishWritingWithCompletionHandler(&block);
                let _ = rx.recv_timeout(Duration::from_secs(30));
                if self.writer.status() == AVAssetWriterStatus::Completed {
                    Ok(())
                } else {
                    Err(self.failed())
                }
            }
        }
    }

    impl Drop for Mp4Writer {
        fn drop(&mut self) {
            if let Some((_, fmt, _)) = &self.audio {
                // SAFETY: 格式描述是 Create 出来的，归我们所有
                unsafe { CFRelease(*fmt) };
            }
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

#[cfg(not(any(windows, target_os = "macos")))]
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

#[cfg(all(test, target_os = "macos"))]
mod mac_tests {
    use super::*;

    /// 离线：2 秒 640×360、30 帧的渐变画面 + 440 Hz 立体声，写成 MP4（不碰屏幕）。
    #[test]
    fn writes_mp4_with_audio() {
        let dir = std::env::temp_dir().join(format!("snapmind-mp4-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.mp4");
        let (w, h, fps, rate) = (640u32, 360u32, 30u32, 48_000u32);
        let mut mp4 = Mp4Writer::create(&path, w, h, fps, Some(rate)).unwrap();
        let frame_dur = Duration::from_secs_f64(1.0 / fps as f64);
        let mut buf = vec![0u8; (w * h * 4) as usize];
        for i in 0..fps * 2 {
            for (j, px) in buf.chunks_exact_mut(4).enumerate() {
                let x = (j as u32 % w + i * 8) % 256;
                px.copy_from_slice(&[x as u8, (255 - x) as u8, 128, 255]);
            }
            let t = frame_dur * i;
            mp4.video(&buf, t, frame_dur).unwrap();
            // 每帧配 1/30 秒的声音
            let n = (rate / fps) as usize;
            let pcm: Vec<i16> = (0..n)
                .flat_map(|k| {
                    let s = ((i as usize * n + k) as f64 / rate as f64 * 440.0 * std::f64::consts::TAU).sin();
                    let v = (s * 8000.0) as i16;
                    [v, v]
                })
                .collect();
            mp4.audio(&pcm, t).unwrap();
        }
        mp4.finish().unwrap();
        let len = std::fs::metadata(&path).unwrap().len();
        eprintln!("写出 {} 字节：{}", len, path.display());
        assert!(len > 10_000);
    }
}
