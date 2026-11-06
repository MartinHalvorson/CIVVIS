// civvis-youtube-master: turn a CIVVIS win clip (two 1728x1028 panels side by
// side, 3456x1028) into a YouTube master: 3840x2160 16:9, 60 fps, HEVC at the
// export session's highest quality, moov atom up front (faststart).
//
// Layout: the panels scale to 3840 wide (1920 each) and sit in the vertical
// middle; the black band above carries the title, the band below names the
// two panels. YouTube gives a 2160p upload its 4K bitrate ladder, which keeps
// the small UI text in both panels legible after its re-encode.
//
//   civvis-youtube-master --in RAW.mp4 --out MASTER.mp4 --title "..."
//       [--subtitle "..."] [--left "..."] [--right "..."] [--start S --dur S] [--cut A:B ...]
//
// Build: swiftc -swift-version 5 -O youtube_master.swift -o civvis-youtube-master
import AVFoundation
import AppKit
import Foundation

func arg(_ name: String) -> String? {
    let a = CommandLine.arguments
    guard let i = a.firstIndex(of: name), i + 1 < a.count else { return nil }
    return a[i + 1]
}

func fail(_ msg: String) -> Never {
    FileHandle.standardError.write((msg + "\n").data(using: .utf8)!)
    exit(1)
}

guard let inPath = arg("--in"), let outPath = arg("--out") else {
    fail("usage: civvis-youtube-master --in RAW.mp4 --out MASTER.mp4 --title T [--subtitle S] [--banner B] [--left L] [--right R] [--start S (--dur S | --end S [--find-board])] [--hud HUD.json] [--cut A:B ...] [--blank-columns A:B ...]")
}
let existingMaster = CommandLine.arguments.contains("--existing-master")
let title = arg("--title") ?? ""
let subtitle = arg("--subtitle") ?? ""
let banner = arg("--banner") ?? ""  // e.g. "DOMINATION VICTORY · TURN 241": fades in over the last seconds
let leftLabel = arg("--left") ?? "Civilization VI"
let rightLabel = arg("--right") ?? "CIVVIS — the AI's view"

let W: CGFloat = 3840, H: CGFloat = 2160
let asset = AVURLAsset(url: URL(fileURLWithPath: inPath))
let sema = DispatchSemaphore(value: 0)
var srcTrack: AVAssetTrack?
var duration = CMTime.zero
Task {
    srcTrack = try? await asset.loadTracks(withMediaType: .video).first
    duration = (try? await asset.load(.duration)) ?? .zero
    sema.signal()
}
sema.wait()
guard let src = srcTrack else { fail("no video track in \(inPath)") }
let natural = src.naturalSize

// The harness holds the end screen, then closes Civ VI while the recorder runs
// on: the last seconds show a black game panel. A full render ends one second
// after the last frame whose game half still shows a picture.
func gamePanelLit(at seconds: Double, _ gen: AVAssetImageGenerator) -> Bool? {
    guard let img = try? gen.copyCGImage(at: CMTime(seconds: seconds, preferredTimescale: 600), actualTime: nil)
    else { return nil }
    let w = 96, h = 32
    guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                              space: CGColorSpaceCreateDeviceRGB(),
                              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
    ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
    guard let data = ctx.data else { return nil }
    let px = data.bindMemory(to: UInt8.self, capacity: w * h * 4)
    var sum = 0, n = 0
    for y in 0..<h { for x in 0..<(w / 2) {  // the left half = the Civ VI panel
        let i = (y * w + x) * 4
        sum += Int(px[i]) + Int(px[i + 1]) + Int(px[i + 2]); n += 3
    } }
    return Double(sum) / Double(max(n, 1)) > 10
}

// Mean RGB of the left (Civ VI) and right (CIVVIS) halves of a frame, for change detection.
func panelMeans(at seconds: Double, _ gen: AVAssetImageGenerator) -> [Double]? {
    guard let img = try? gen.copyCGImage(at: CMTime(seconds: seconds, preferredTimescale: 600), actualTime: nil)
    else { return nil }
    let w = 96, h = 32
    guard let ctx = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: w * 4,
                              space: CGColorSpaceCreateDeviceRGB(),
                              bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
    ctx.draw(img, in: CGRect(x: 0, y: 0, width: w, height: h))
    guard let data = ctx.data else { return nil }
    let px = data.bindMemory(to: UInt8.self, capacity: w * h * 4)
    var out: [Double] = []
    for (x0, x1) in [(4, w / 2 - 4), (w / 2 + 4, w - 4)] {
        var r = 0, g = 0, b = 0, n = 0
        for y in 0..<h { for x in x0..<x1 {
            let i = (y * w + x) * 4
            r += Int(px[i]); g += Int(px[i + 1]); b += Int(px[i + 2]); n += 1
        } }
        out += [Double(r) / Double(n), Double(g) / Double(n), Double(b) / Double(n)]
    }
    return out
}

var range = CMTimeRange(start: .zero, duration: duration)
if let s = arg("--start").flatMap(Double.init), let e = arg("--end").flatMap(Double.init) {
    // Operator 2026-10-14: turn 1 to the final turn, nothing more. With --find-board
    // the start skips leading footage where BOTH panels are frozen: the leader-intro
    // loading screen on the Civ VI side while the CIVVIS page still shows the previous
    // game (clips before the 10-13 playout fix froze both). The first moment either
    // panel moves is the start. A live panel moves within seconds, so live turn-1
    // footage is never cut.
    var start = s
    if CommandLine.arguments.contains("--find-board") {
        let gen = AVAssetImageGenerator(asset: asset)
        gen.maximumSize = CGSize(width: 192, height: 64)
        gen.requestedTimeToleranceBefore = .zero
        gen.requestedTimeToleranceAfter = .zero
        func moved(_ m: [Double], _ base: [Double]) -> Bool {
            let left = abs(m[0] - base[0]) + abs(m[1] - base[1]) + abs(m[2] - base[2])
            let right = abs(m[3] - base[3]) + abs(m[4] - base[4]) + abs(m[5] - base[5])
            return left > 6 || right > 6
        }
        func movedSide(_ m: [Double], _ base: [Double], right: Bool) -> Bool {
            let o = right ? 3 : 0
            return abs(m[o] - base[o]) + abs(m[o + 1] - base[o + 1]) + abs(m[o + 2] - base[o + 2]) > 6
        }
        if let base = panelMeans(at: s, gen) {
            // The CIVVIS page is what lags: after a new game starts it can still
            // show the previous game for a few seconds (2026-10-15 101527Z: Civ VI
            // on turn 1 at 4 s, CIVVIS on the old turn 214 until ~5.5 s). So the
            // start is the moment the CIVVIS panel changes, plus a second for the
            // page's own switch-over. Only if the CIVVIS panel never changes
            // within the search does the first change of either panel decide.
            var rightAt = -1.0, anyAt = -1.0
            var t = s + 1
            while t < min(e - 30, s + 900) {
                if let m = panelMeans(at: t, gen) {
                    if anyAt < 0 && moved(m, base) { anyAt = t }
                    if movedSide(m, base, right: true) { rightAt = t; break }
                }
                t += 1
            }
            func refine(_ coarse: Double, right: Bool) -> Double {
                var f = coarse - 1 + 0.1
                while f < coarse, let m = panelMeans(at: f, gen),
                      !(right ? movedSide(m, base, right: true) : moved(m, base)) { f += 0.1 }
                return min(f, coarse)
            }
            if rightAt > 0 {
                start = min(refine(rightAt, right: true) + 1.0, e - 31)
                print(String(format: "CIVVIS shows this game from %.1fs; starting there", start))
            } else if anyAt > 0 {
                start = refine(anyAt, right: false)
                if start > s + 0.05 {
                    print(String(format: "frozen until %.1fs (loading screen / previous game); starting there", start))
                }
            }
            // Never open on a black panel: Civ VI blanks for a moment when its
            // window moves into the operator's saved frame after setup.
            let opened = start
            var f = start
            while f < opened + 5, let m = panelMeans(at: f, gen),
                  (m[0] + m[1] + m[2]) / 3 < 12 || (m[3] + m[4] + m[5]) / 3 < 12 { f += 0.1 }
            if f > opened + 0.05 && f < opened + 5 {
                start = f
                print(String(format: "a panel is black until %.1fs; starting there", start))
            }
        }
    }
    range = CMTimeRange(start: CMTime(seconds: start, preferredTimescale: 600),
                        duration: CMTime(seconds: max(1, e - start), preferredTimescale: 600))
} else if let s = arg("--start").flatMap(Double.init), let d = arg("--dur").flatMap(Double.init) {
    range = CMTimeRange(start: CMTime(seconds: s, preferredTimescale: 600),
                        duration: CMTime(seconds: d, preferredTimescale: 600))
} else {
    let gen = AVAssetImageGenerator(asset: asset)
    gen.maximumSize = CGSize(width: 432, height: 128)
    gen.requestedTimeToleranceBefore = CMTime(seconds: 0.25, preferredTimescale: 600)
    gen.requestedTimeToleranceAfter = CMTime(seconds: 0.25, preferredTimescale: 600)
    let total = duration.seconds
    var t = total - 0.2
    while t > max(0, total - 120) {
        if gamePanelLit(at: t, gen) == true { break }
        t -= 0.5
    }
    if t > max(0, total - 120) && t < total - 1.5 {
        let end = min(total, t + 1.0)
        print(String(format: "trimmed the dark tail: %.1fs → %.1fs", total, end))
        range = CMTimeRange(start: .zero, duration: CMTime(seconds: end, preferredTimescale: 600))
    }
}

// --cut A:B (repeatable, seconds of the input): a span the quality receipt drops because
// the live game was held there (operator 2026-10-30: the Lv 7 Diplomatic master sat 2 min
// 47 s on the turn-6 Animal Husbandry screen while the recorder's pause held the game).
// Nothing played inside a cut; the kept pieces butt together, each at 1x.
func span(_ a: Double, _ b: Double) -> CMTimeRange {
    CMTimeRange(start: CMTime(seconds: a, preferredTimescale: 600),
                end: CMTime(seconds: b, preferredTimescale: 600))
}
var cuts: [(Double, Double)] = []
for (i, a) in CommandLine.arguments.enumerated() where a == "--cut" {
    let v = i + 1 < CommandLine.arguments.count ? CommandLine.arguments[i + 1] : ""
    let p = v.split(separator: ":").compactMap { Double($0) }
    guard p.count == 2, p[1] > p[0] else { fail("bad --cut '\(v)' (want A:B in seconds of the input)") }
    cuts.append((p[0], p[1]))
}
var pieces: [CMTimeRange] = []
var cursor = range.start.seconds
for (a, b) in cuts.sorted(by: { $0.0 < $1.0 }) where b > cursor && a < range.end.seconds {
    if a > cursor { pieces.append(span(cursor, a)) }
    cursor = max(cursor, b)
}
if cursor < range.end.seconds { pieces.append(span(cursor, range.end.seconds)) }

let comp = AVMutableComposition()
guard let track = comp.addMutableTrack(withMediaType: .video, preferredTrackID: kCMPersistentTrackID_Invalid) else {
    fail("cannot add a composition track")
}
var outDuration = CMTime.zero
for piece in pieces {
    do { try track.insertTimeRange(piece, of: src, at: outDuration) } catch { fail("insert: \(error)") }
    outDuration = outDuration + piece.duration
}
if let hold = arg("--hold-final").flatMap(Double.init), hold > 0 {
    let frame = CMTime(value: 1, timescale: 60)
    let lastFrame = CMTimeRange(start: range.end - frame, duration: frame)
    for _ in 0..<Int((hold * 60).rounded()) {
        try track.insertTimeRange(lastFrame, of: src, at: outDuration)
        outDuration = outDuration + frame
    }
}
if pieces.count > 1 {
    print(String(format: "cut %.1fs of held game in %d place(s): %.1fs → %.1fs", range.duration.seconds - outDuration.seconds,
                 pieces.count - 1, range.duration.seconds, outDuration.seconds))
}

// Scale the side-by-side panels to the full 3840 width, centred vertically.
let scale = W / natural.width
let videoH = natural.height * scale
let top = (H - videoH) / 2
let layer = AVMutableVideoCompositionLayerInstruction(assetTrack: track)
layer.setTransform(CGAffineTransform(scaleX: scale, y: scale).concatenating(
    CGAffineTransform(translationX: 0, y: top)), at: .zero)
let instr = AVMutableVideoCompositionInstruction()
instr.timeRange = CMTimeRange(start: .zero, duration: outDuration)
instr.layerInstructions = [layer]
instr.backgroundColor = NSColor.black.cgColor

let vc = AVMutableVideoComposition()
vc.renderSize = CGSize(width: W, height: H)
vc.frameDuration = CMTime(value: 1, timescale: 60)
vc.instructions = [instr]

// Title and panel captions in the black bands. Drawn once into a transparent
// overlay image: CATextLayer does not reliably render in AVFoundation's offline
// Core Animation pass on macOS, a layer with bitmap contents always does.
let band = (H - videoH) / 2
let gold = NSColor(calibratedRed: 0.93, green: 0.80, blue: 0.45, alpha: 1)
let grey = NSColor(calibratedWhite: 0.80, alpha: 1)
func draw(_ s: String, size: CGFloat, weight: NSFont.Weight, color: NSColor, centreX: CGFloat, y: CGFloat, maxW: CGFloat) {
    guard !s.isEmpty else { return }
    var font = NSFont.systemFont(ofSize: size, weight: weight)
    var attrs: [NSAttributedString.Key: Any] = [.font: font, .foregroundColor: color]
    var w = (s as NSString).size(withAttributes: attrs).width
    if w > maxW {  // shrink to fit rather than clip
        font = NSFont.systemFont(ofSize: size * maxW / w, weight: weight)
        attrs[.font] = font
        w = (s as NSString).size(withAttributes: attrs).width
    }
    (s as NSString).draw(at: CGPoint(x: centreX - w / 2, y: y), withAttributes: attrs)
}
let cs = CGColorSpaceCreateDeviceRGB()
guard let ctx = CGContext(data: nil, width: Int(W), height: Int(H), bitsPerComponent: 8, bytesPerRow: 0,
                          space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { fail("no overlay context") }
NSGraphicsContext.saveGraphicsState()
NSGraphicsContext.current = NSGraphicsContext(cgContext: ctx, flipped: false)
draw(title, size: 120, weight: .bold, color: gold, centreX: W / 2, y: H - band + band * 0.40, maxW: W - 240)
draw(subtitle, size: 64, weight: .regular, color: grey, centreX: W / 2, y: H - band + band * 0.14, maxW: W - 240)
// --hud FILE (JSON from civvis_video_story.py): {"items": [{"t", "turn", "info", "race",
// "progress"}], "callouts": [{"t", "dur", "text"}]}. Times are seconds into this
// master. With a HUD the bottom band carries, top to bottom: the panel captions,
// a per-turn scoreboard (turn, era, the victory race with a progress bar) and the
// milestone call-outs. Without one the band keeps the two captions as before.
var hudItems: [[String: Any]] = []
var callouts: [[String: Any]] = []
if let hudPath = arg("--hud") {
    guard let data = FileManager.default.contents(atPath: hudPath),
          let obj = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
        fail("unreadable --hud \(hudPath)")
    }
    hudItems = (obj["items"] as? [[String: Any]] ?? []).sorted { ($0["t"] as? Double ?? 0) < ($1["t"] as? Double ?? 0) }
    callouts = obj["callouts"] as? [[String: Any]] ?? []
}
let captionY = hudItems.isEmpty ? band * 0.55 : band * 0.80
let captionSize: CGFloat = hudItems.isEmpty ? 64 : 52
draw(leftLabel, size: captionSize, weight: .semibold, color: grey, centreX: W / 4, y: captionY, maxW: W / 2 - 120)
draw(rightLabel, size: captionSize, weight: .semibold, color: grey, centreX: 3 * W / 4, y: captionY, maxW: W / 2 - 120)
NSGraphicsContext.restoreGraphicsState()
guard let overlayImage = ctx.makeImage() else { fail("no overlay image") }
let parent = CALayer()
parent.frame = CGRect(x: 0, y: 0, width: W, height: H)
let videoLayer = CALayer()
videoLayer.frame = parent.frame
parent.addSublayer(videoLayer)
let overlay = CALayer()
overlay.frame = parent.frame
overlay.contents = overlayImage
if !existingMaster { parent.addSublayer(overlay) }

// --blank-columns A:B (source pixels, repeatable): paint these input columns black over the
// whole video band. Operator 2026-10-31 on the Lv 7 Diplomatic master (204643Z): "the civvis
// side appears to have 2 overlapping civvis games". Civ VI's window was 902 pt wide while the
// CIVVIS page began at 864 pt; with Chrome raised above Civ, the old shared capture filter put
// the page's left 38 pt into the right edge of the Civ VI slot (input columns 1728-1804), right
// beside the CIVVIS slot. The Civ VI picture under it was never captured, so it stays black.
for (i, a) in CommandLine.arguments.enumerated() where a == "--blank-columns" {
    let v = i + 1 < CommandLine.arguments.count ? CommandLine.arguments[i + 1] : ""
    let p = v.split(separator: ":").compactMap { Double($0) }
    guard p.count == 2, p[1] > p[0], p[0] >= 0, p[1] <= natural.width else {
        fail("bad --blank-columns '\(v)' (want A:B in input pixel columns)")
    }
    let mask = CALayer()
    mask.frame = CGRect(x: floor(p[0] * scale), y: top, width: ceil((p[1] - p[0]) * scale), height: videoH)
    mask.backgroundColor = NSColor.black.cgColor
    parent.addSublayer(mask)
    print(String(format: "blanked input columns %.0f-%.0f", p[0], p[1]))
}

// One bitmap per scoreboard state, swapped on a single layer by a discrete
// keyframe animation over the whole master; each call-out is its own layer that
// fades in and out. Everything stays inside the bottom band, off both panels.
func bitmap(_ w: Int, _ h: Int, _ body: () -> Void) -> CGImage? {
    guard let c = CGContext(data: nil, width: w, height: h, bitsPerComponent: 8, bytesPerRow: 0,
                            space: CGColorSpaceCreateDeviceRGB(),
                            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) else { return nil }
    NSGraphicsContext.saveGraphicsState()
    NSGraphicsContext.current = NSGraphicsContext(cgContext: c, flipped: false)
    body()
    NSGraphicsContext.restoreGraphicsState()
    return c.makeImage()
}
func runWidth(_ parts: [(String, NSFont)]) -> CGFloat {
    parts.reduce(0) { $0 + ($1.0 as NSString).size(withAttributes: [.font: $1.1]).width }
}
let total = outDuration.seconds
if !hudItems.isEmpty && total > 0 && !existingMaster {
    let sw = Int(W - 200), sh = 170
    let white = NSColor(calibratedWhite: 0.96, alpha: 1)
    var images: [CGImage] = []
    var keyTimes: [NSNumber] = []
    for item in hudItems {
        let turnText = item["turn"] as? String ?? ""
        let info = item["info"] as? String ?? ""
        let race = item["race"] as? String ?? ""
        let progress = item["progress"] as? Double
        guard let img = bitmap(sw, sh, {
            let box = NSBezierPath(roundedRect: NSRect(x: 0, y: 0, width: sw, height: sh), xRadius: 34, yRadius: 34)
            NSColor(calibratedWhite: 0.10, alpha: 0.92).setFill()
            box.fill()
            NSColor(calibratedRed: 0.93, green: 0.80, blue: 0.45, alpha: 0.55).setStroke()
            box.lineWidth = 3
            box.stroke()
            var size: CGFloat = 70
            let sep = "   ·   "
            func fonts(_ s: CGFloat) -> [(String, NSFont)] {
                [(turnText, NSFont.systemFont(ofSize: s, weight: .heavy)),
                 (info.isEmpty ? "" : sep + info, NSFont.systemFont(ofSize: s, weight: .medium)),
                 (race.isEmpty ? "" : sep + race, NSFont.systemFont(ofSize: s, weight: .bold))]
            }
            var parts = fonts(size)
            let maxW = CGFloat(sw) - 120
            if runWidth(parts) > maxW { size *= maxW / runWidth(parts); parts = fonts(size) }
            var x = (CGFloat(sw) - runWidth(parts)) / 2
            let y = progress == nil ? CGFloat(sh) / 2 - size * 0.62 : CGFloat(sh) * 0.60 - size * 0.62
            for (i, part) in parts.enumerated() where !part.0.isEmpty {
                let colour = i == 1 ? white : gold
                let attrs: [NSAttributedString.Key: Any] = [.font: part.1, .foregroundColor: colour]
                (part.0 as NSString).draw(at: CGPoint(x: x, y: y), withAttributes: attrs)
                x += (part.0 as NSString).size(withAttributes: attrs).width
            }
            if let p = progress {
                let bw: CGFloat = CGFloat(sw) * 0.56, bh: CGFloat = 16
                let bx = (CGFloat(sw) - bw) / 2, by: CGFloat = 22
                NSColor(calibratedWhite: 0.30, alpha: 1).setFill()
                NSBezierPath(roundedRect: NSRect(x: bx, y: by, width: bw, height: bh), xRadius: 8, yRadius: 8).fill()
                gold.setFill()
                NSBezierPath(roundedRect: NSRect(x: bx, y: by, width: max(bh, bw * CGFloat(min(1, max(0, p)))), height: bh),
                             xRadius: 8, yRadius: 8).fill()
            }
        }) else { continue }
        images.append(img)
        keyTimes.append(NSNumber(value: min(1, max(0, (item["t"] as? Double ?? 0) / total))))
    }
    if !images.isEmpty {
        keyTimes[0] = 0
        // A discrete animation takes one more key time than values: value i shows from
        // keyTimes[i] to keyTimes[i+1]. Without the closing 1 the last state never showed
        // (the final turns read one turn stale, 204643Z).
        keyTimes.append(1)
        let strip = CALayer()
        strip.frame = CGRect(x: 100, y: band * 0.30, width: CGFloat(sw), height: CGFloat(sh))
        strip.contents = images[0]
        let swap = CAKeyframeAnimation(keyPath: "contents")
        swap.values = images
        swap.keyTimes = keyTimes
        swap.calculationMode = .discrete
        swap.beginTime = AVCoreAnimationBeginTimeAtZero
        swap.duration = total
        swap.fillMode = .forwards
        swap.isRemovedOnCompletion = false
        strip.add(swap, forKey: "scoreboard")
        parent.addSublayer(strip)
    }
    for c in callouts {
        guard let text = c["text"] as? String, !text.isEmpty, let t = c["t"] as? Double, t < total else { continue }
        let dur = min(c["dur"] as? Double ?? 7, total - t)
        guard dur > 1.5 else { continue }
        let font = NSFont.systemFont(ofSize: 60, weight: .semibold)
        let tw = min((text as NSString).size(withAttributes: [.font: font]).width, W - 400)
        let pw = Int(tw + 140), ph = 104
        guard let img = bitmap(pw, ph, {
            let pill = NSBezierPath(roundedRect: NSRect(x: 2, y: 2, width: pw - 4, height: ph - 4), xRadius: 50, yRadius: 50)
            NSColor(calibratedRed: 0.20, green: 0.15, blue: 0.04, alpha: 0.95).setFill()
            pill.fill()
            gold.setStroke()
            pill.lineWidth = 4
            pill.stroke()
            draw(text, size: 60, weight: .semibold, color: NSColor(calibratedWhite: 0.98, alpha: 1),
                 centreX: CGFloat(pw) / 2, y: CGFloat(ph) / 2 - 37, maxW: CGFloat(pw) - 100)
        }) else { continue }
        let layerC = CALayer()
        layerC.frame = CGRect(x: (W - CGFloat(pw)) / 2, y: band * 0.04, width: CGFloat(pw), height: CGFloat(ph))
        layerC.contents = img
        layerC.opacity = 0
        let fade = CAKeyframeAnimation(keyPath: "opacity")
        fade.values = [0, 1, 1, 0]
        fade.keyTimes = [0, NSNumber(value: 0.5 / dur), NSNumber(value: 1 - 0.6 / dur), 1]
        fade.beginTime = AVCoreAnimationBeginTimeAtZero + max(0.05, t)
        fade.duration = dur
        fade.isRemovedOnCompletion = true
        layerC.add(fade, forKey: "callout")
        parent.addSublayer(layerC)
    }
    print("hud: \(images.count) scoreboard states, \(callouts.count) call-outs")
}

// End card: the victory banner fades in over the panels for the last few
// seconds, so the video ends on the result (the game's own victory screen is
// not in frame by then).
if !banner.isEmpty && CommandLine.arguments.contains("--victory-banner") {
    let bw = Int(W * 0.78), bh = 420
    if let bctx = CGContext(data: nil, width: bw, height: bh, bitsPerComponent: 8, bytesPerRow: 0,
                            space: CGColorSpaceCreateDeviceRGB(),
                            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue) {
        NSGraphicsContext.saveGraphicsState()
        NSGraphicsContext.current = NSGraphicsContext(cgContext: bctx, flipped: false)
        let box = NSBezierPath(roundedRect: NSRect(x: 0, y: 0, width: bw, height: bh), xRadius: 48, yRadius: 48)
        NSColor(calibratedWhite: 0.04, alpha: 0.86).setFill()
        box.fill()
        gold.setStroke()
        box.lineWidth = 10
        box.stroke()
        draw(banner, size: 190, weight: .heavy, color: gold, centreX: CGFloat(bw) / 2,
             y: CGFloat(bh) / 2 - 95, maxW: CGFloat(bw) - 160)
        NSGraphicsContext.restoreGraphicsState()
        if let bimg = bctx.makeImage() {
            let card = CALayer()
            card.frame = CGRect(x: (W - CGFloat(bw)) / 2, y: (H - CGFloat(bh)) / 2,
                                width: CGFloat(bw), height: CGFloat(bh))
            card.contents = bimg
            card.opacity = 0
            let fade = CABasicAnimation(keyPath: "opacity")
            fade.fromValue = 0
            fade.toValue = 1
            fade.beginTime = AVCoreAnimationBeginTimeAtZero + max(0.1, outDuration.seconds - 6)
            fade.duration = 0.8
            fade.fillMode = .forwards
            fade.isRemovedOnCompletion = false
            card.add(fade, forKey: "fade-in")
            parent.addSublayer(card)
        }
    }
}
// Confirmed final result: hide the stale pre-victory board and its counters.
if CommandLine.arguments.contains("--confirmed-result-card"),
   let result = hudItems.last, result["confirmed_result"] as? Bool == true {
    let resultAt = result["t"] as? Double ?? total
    let detail = result["result_detail"] as? String ?? ""
    let resultTitle = (result["race"] as? String ?? "Victory").components(separatedBy: " · ").first ?? "Victory"
    let turnLabel = result["turn"] as? String ?? ""
    let cw = Int(W), ch = Int(H)
    if let img = bitmap(cw, ch, {
        NSColor(calibratedWhite: 0.025, alpha: 1).setFill()
        NSRect(x: 0, y: 0, width: cw, height: ch).fill()
        draw("CIVVIS · FINAL RESULT" + (turnLabel.isEmpty ? "" : " · " + turnLabel),
             size: 70, weight: .semibold, color: grey, centreX: W / 2, y: H * 0.69, maxW: W - 300)
        draw(resultTitle, size: 210, weight: .bold, color: gold,
             centreX: W / 2, y: H * 0.51, maxW: W - 300)
        draw(detail, size: 100, weight: .semibold, color: .white,
             centreX: W / 2, y: H * 0.38, maxW: W - 300)
        draw(result["result_note"] as? String ?? "Victory confirmed · counters show the winning minimum",
             size: 55, weight: .regular, color: grey, centreX: W / 2, y: H * 0.24, maxW: W - 300)
    }) {
        let card = CALayer()
        card.frame = CGRect(x: 0, y: 0, width: W, height: H)
        card.contents = img; card.opacity = 0
        let reveal = CABasicAnimation(keyPath: "opacity")
        reveal.fromValue = 0; reveal.toValue = 1
        reveal.beginTime = AVCoreAnimationBeginTimeAtZero + max(0.001, resultAt)
        reveal.duration = 0.001; reveal.fillMode = .forwards; reveal.isRemovedOnCompletion = false
        card.add(reveal, forKey: "confirmed-result")
        parent.addSublayer(card)
    }
}

vc.animationTool = AVVideoCompositionCoreAnimationTool(postProcessingAsVideoLayer: videoLayer, in: parent)

let outURL = URL(fileURLWithPath: outPath)
try? FileManager.default.removeItem(at: outURL)
guard let ex = AVAssetExportSession(asset: comp, presetName: AVAssetExportPresetHEVCHighestQuality) else {
    fail("no HEVC export session")
}
ex.outputURL = outURL
ex.outputFileType = .mp4
ex.videoComposition = vc
ex.shouldOptimizeForNetworkUse = true  // moov first: YouTube starts processing during upload

let started = Date()
let done = DispatchSemaphore(value: 0)
ex.exportAsynchronously { done.signal() }
var last = -1
while done.wait(timeout: .now() + 15) == .timedOut {
    let pct = Int(ex.progress * 100)
    if pct != last {
        print(String(format: "progress %d%% after %.0fs", pct, Date().timeIntervalSince(started)))
        fflush(stdout)
        last = pct
    }
}
if ex.status != .completed {
    fail("export failed: \(String(describing: ex.error))")
}
print(String(format: "done %.1fs of video in %.0fs → %@", outDuration.seconds,
             Date().timeIntervalSince(started), outPath))
