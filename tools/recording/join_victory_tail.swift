// Remux the untouched original prefix and a repaired ending without reencoding gameplay.
import AVFoundation
import Foundation
let args = CommandLine.arguments
if args.count != 5 { fatalError("source tail prefixSeconds output") }
let source = AVURLAsset(url: URL(fileURLWithPath: args[1]))
let tail = AVURLAsset(url: URL(fileURLWithPath: args[2]))
let cut = CMTime(seconds: Double(args[3])!, preferredTimescale: 600)
let comp = AVMutableComposition()
let track = comp.addMutableTrack(withMediaType: .video, preferredTrackID: kCMPersistentTrackID_Invalid)!
try track.insertTimeRange(CMTimeRange(start: .zero, duration: cut), of: source.tracks(withMediaType: .video)[0], at: .zero)
try track.insertTimeRange(CMTimeRange(start: .zero, duration: tail.duration), of: tail.tracks(withMediaType: .video)[0], at: cut)
let ex = AVAssetExportSession(asset: comp, presetName: AVAssetExportPresetPassthrough)!
ex.outputURL = URL(fileURLWithPath: args[4]); ex.outputFileType = .mp4
ex.shouldOptimizeForNetworkUse = true
let done = DispatchSemaphore(value: 0)
ex.exportAsynchronously { done.signal() }; done.wait()
if ex.status != .completed { fatalError("join failed: \(String(describing: ex.error))") }
print("joined \(comp.duration.seconds) seconds")
