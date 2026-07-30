#!/usr/bin/env swift

import AppKit
import Foundation

// Builds the spectator's compact command-map atlas from the Civilization VI
// Civilopedia icons archived by the Civilization Wiki. The Civilopedia cards
// all share one background; taking a low per-pixel percentile across the full
// unit set recovers that background, leaving the game's white unit glyphs.
//
// Usage:
//   swift tools/civ6_unit_flags.swift \
//     web/assets/civ6-unit-flags.png web/assets/civ6-unit-flags.json

struct UnitSource {
    let type: String
    let title: String
}

let units = [
    UnitSource(type: "aircraft_carrier", title: "Aircraft Carrier icon (Civ6).png"),
    UnitSource(type: "anti_air_gun", title: "Anti-Air Gun icon (Civ6).png"),
    UnitSource(type: "apostle", title: "Apostle icon (Civ6).png"),
    UnitSource(type: "archaeologist", title: "Archaeologist icon (Civ6).png"),
    UnitSource(type: "archer", title: "Archer icon (Civ6).png"),
    UnitSource(type: "artillery", title: "Artillery icon (Civ6).png"),
    UnitSource(type: "at_crew", title: "AT Crew icon (Civ6).png"),
    UnitSource(type: "battering_ram", title: "Battering Ram icon (Civ6).png"),
    UnitSource(type: "battleship", title: "Battleship icon (Civ6).png"),
    UnitSource(type: "biplane", title: "Biplane icon (Civ6).png"),
    UnitSource(type: "bombard", title: "Bombard icon (Civ6).png"),
    UnitSource(type: "bomber", title: "Bomber icon (Civ6).png"),
    UnitSource(type: "builder", title: "Builder icon (Civ6).png"),
    UnitSource(type: "caravel", title: "Caravel icon (Civ6).png"),
    UnitSource(type: "catapult", title: "Catapult icon (Civ6).png"),
    UnitSource(type: "cavalry", title: "Cavalry icon (Civ6).png"),
    UnitSource(type: "courser", title: "Courser icon (Civ6).png"),
    UnitSource(type: "crossbowman", title: "Crossbowman icon (Civ6).png"),
    UnitSource(type: "crouching_tiger", title: "Crouching Tiger icon (Civ6).png"),
    UnitSource(type: "cuirassier", title: "Cuirassier icon (Civ6).png"),
    UnitSource(type: "destroyer", title: "Destroyer icon (Civ6).png"),
    UnitSource(type: "drone", title: "Drone icon (Civ6).png"),
    UnitSource(type: "eagle_warrior", title: "Eagle Warrior icon (Civ6).png"),
    UnitSource(type: "field_cannon", title: "Field Cannon icon (Civ6).png"),
    UnitSource(type: "fighter", title: "Fighter icon (Civ6).png"),
    UnitSource(type: "frigate", title: "Frigate icon (Civ6).png"),
    UnitSource(type: "galley", title: "Galley icon (Civ6).png"),
    UnitSource(type: "giant_death_robot", title: "Giant Death Robot icon (Civ6).png"),
    UnitSource(type: "guru", title: "Guru icon (Civ6).png"),
    UnitSource(type: "heavy_chariot", title: "Heavy Chariot icon (Civ6).png"),
    UnitSource(type: "helicopter", title: "Helicopter icon (Civ6).png"),
    UnitSource(type: "hoplite", title: "Hoplite icon (Civ6).png"),
    UnitSource(type: "horseman", title: "Horseman icon (Civ6).png"),
    UnitSource(type: "infantry", title: "Infantry icon (Civ6).png"),
    UnitSource(type: "inquisitor", title: "Inquisitor icon (Civ6).png"),
    UnitSource(type: "ironclad", title: "Ironclad icon (Civ6).png"),
    UnitSource(type: "jet_bomber", title: "Jet Bomber icon (Civ6).png"),
    UnitSource(type: "jet_fighter", title: "Jet Fighter icon (Civ6).png"),
    UnitSource(type: "knight", title: "Knight icon (Civ6).png"),
    UnitSource(type: "legion", title: "Legion icon (Civ6).png"),
    UnitSource(type: "line_infantry", title: "Line Infantry icon (Civ6).png"),
    UnitSource(type: "machine_gun", title: "Machine Gun icon (Civ6).png"),
    UnitSource(type: "man_at_arms", title: "Man-At-Arms icon (Civ6).png"),
    UnitSource(type: "maryannu_chariot_archer", title: "Maryannu Chariot Archer icon (Civ6).png"),
    UnitSource(type: "mechanized_infantry", title: "Mechanized Infantry icon (Civ6).png"),
    UnitSource(type: "medic", title: "Medic icon (Civ6).png"),
    UnitSource(type: "military_engineer", title: "Military Engineer icon (Civ6).png"),
    UnitSource(type: "missile_cruiser", title: "Missile Cruiser icon (Civ6).png"),
    UnitSource(type: "missionary", title: "Missionary icon (Civ6).png"),
    UnitSource(type: "mobile_sam", title: "Mobile SAM icon (Civ6).png"),
    UnitSource(type: "modern_armor", title: "Modern Armor icon (Civ6).png"),
    UnitSource(type: "modern_at", title: "Modern AT icon (Civ6).png"),
    UnitSource(type: "musketman", title: "Musketman icon (Civ6).png"),
    UnitSource(type: "naturalist", title: "Naturalist icon (Civ6).png"),
    UnitSource(type: "nuclear_submarine", title: "Nuclear Submarine icon (Civ6).png"),
    UnitSource(type: "observation_balloon", title: "Observation Balloon icon (Civ6).png"),
    UnitSource(type: "pike_and_shot", title: "Pike and Shot icon (Civ6).png"),
    UnitSource(type: "pikeman", title: "Pikeman icon (Civ6).png"),
    UnitSource(type: "pitati_archer", title: "Pítati Archer icon (Civ6).png"),
    UnitSource(type: "privateer", title: "Privateer icon (Civ6).png"),
    UnitSource(type: "quadrireme", title: "Quadrireme icon (Civ6).png"),
    UnitSource(type: "ranger", title: "Ranger icon (Civ6).png"),
    UnitSource(type: "rock_band", title: "Rock Band icon (Civ6).png"),
    UnitSource(type: "rocket_artillery", title: "Rocket Artillery icon (Civ6).png"),
    UnitSource(type: "saka_horse_archer", title: "Saka Horse Archer icon (Civ6).png"),
    UnitSource(type: "scout", title: "Scout icon (Civ6).png"),
    UnitSource(type: "settler", title: "Settler icon (Civ6).png"),
    UnitSource(type: "siege_tower", title: "Siege Tower icon (Civ6).png"),
    UnitSource(type: "skirmisher", title: "Skirmisher icon (Civ6).png"),
    UnitSource(type: "slinger", title: "Slinger icon (Civ6).png"),
    UnitSource(type: "spearman", title: "Spearman icon (Civ6).png"),
    UnitSource(type: "spec_ops", title: "Spec Ops icon (Civ6).png"),
    UnitSource(type: "spy", title: "Spy icon (Civ6).png"),
    UnitSource(type: "submarine", title: "Submarine icon (Civ6).png"),
    UnitSource(type: "supply_convoy", title: "Supply Convoy icon (Civ6).png"),
    UnitSource(type: "swordsman", title: "Swordsman icon (Civ6).png"),
    UnitSource(type: "tagma", title: "Tagma icon (Civ6).png"),
    UnitSource(type: "tank", title: "Tank icon (Civ6).png"),
    UnitSource(type: "trader", title: "Trader icon (Civ6).png"),
    UnitSource(type: "trebuchet", title: "Trebuchet icon (Civ6).png"),
    UnitSource(type: "war_cart", title: "War-Cart icon (Civ6).png"),
    UnitSource(type: "warrior", title: "Warrior icon (Civ6).png"),
    UnitSource(type: "warrior_monk", title: "Warrior Monk icon (Civ6).png"),
]

let sourceSize = 256
let cellSize = 64
let columns = 12
let rows = (units.count + columns - 1) / columns

guard CommandLine.arguments.count == 3 else {
    FileHandle.standardError.write(Data("expected atlas and manifest output paths\n".utf8))
    exit(2)
}

func wikiImageURL(for title: String) throws -> URL {
    var parts = URLComponents(string: "https://civilization.fandom.com/api.php")!
    parts.queryItems = [
        URLQueryItem(name: "action", value: "query"),
        URLQueryItem(name: "format", value: "json"),
        URLQueryItem(name: "titles", value: "File:\(title)"),
        URLQueryItem(name: "prop", value: "imageinfo"),
        URLQueryItem(name: "iiprop", value: "url"),
    ]
    let data = try Data(contentsOf: parts.url!)
    let root = try JSONSerialization.jsonObject(with: data) as! [String: Any]
    let query = root["query"] as! [String: Any]
    let pages = query["pages"] as! [String: Any]
    guard let page = pages.values.first as? [String: Any],
          let info = (page["imageinfo"] as? [[String: Any]])?.first,
          let raw = info["url"] as? String,
          var original = URLComponents(string: raw) else {
        throw NSError(domain: "civ6-unit-flags", code: 1,
                      userInfo: [NSLocalizedDescriptionKey: "missing wiki image: \(title)"])
    }
    var items = original.queryItems ?? []
    items.append(URLQueryItem(name: "format", value: "original"))
    original.queryItems = items
    return original.url!
}

func fetch(_ source: UnitSource, cache: URL) throws -> Data {
    let target = cache.appendingPathComponent(source.type + ".png")
    if let cached = try? Data(contentsOf: target), cached.count > 1_000 {
        return cached
    }
    let remote = try wikiImageURL(for: source.title)
    var lastError: Error?
    for attempt in 0..<3 {
        do {
            let data = try Data(contentsOf: remote)
            guard data.count > 1_000 else {
                throw NSError(domain: "civ6-unit-flags", code: 2,
                              userInfo: [NSLocalizedDescriptionKey: "empty wiki image: \(source.title)"])
            }
            try data.write(to: target, options: .atomic)
            return data
        } catch {
            lastError = error
            if attempt < 2 { Thread.sleep(forTimeInterval: Double(attempt + 1)) }
        }
    }
    throw lastError!
}

func luminance(_ data: Data, title: String) throws -> [UInt8] {
    guard let image = NSImage(data: data),
          let bitmap = NSBitmapImageRep(data: image.tiffRepresentation!) else {
        throw NSError(domain: "civ6-unit-flags", code: 3,
                      userInfo: [NSLocalizedDescriptionKey: "cannot decode \(title)"])
    }
    guard bitmap.pixelsWide == sourceSize, bitmap.pixelsHigh == sourceSize else {
        throw NSError(domain: "civ6-unit-flags", code: 4,
                      userInfo: [NSLocalizedDescriptionKey:
                        "\(title) is \(bitmap.pixelsWide)x\(bitmap.pixelsHigh), expected 256x256"])
    }
    var result = [UInt8](repeating: 0, count: sourceSize * sourceSize)
    for y in 0..<sourceSize {
        for x in 0..<sourceSize {
            let color = bitmap.colorAt(x: x, y: y)!.usingColorSpace(.deviceRGB)!
            let red = Int((color.redComponent * 255).rounded())
            let green = Int((color.greenComponent * 255).rounded())
            let blue = Int((color.blueComponent * 255).rounded())
            result[y * sourceSize + x] = UInt8((54 * red + 183 * green + 19 * blue) >> 8)
        }
    }
    return result
}

let manager = FileManager.default
let cache = manager.temporaryDirectory.appendingPathComponent("civvis-civ6-unit-icons", isDirectory: true)
try manager.createDirectory(at: cache, withIntermediateDirectories: true)

var images: [[UInt8]] = []
for (index, source) in units.enumerated() {
    let data = try fetch(source, cache: cache)
    images.append(try luminance(data, title: source.title))
    print("[\(index + 1)/\(units.count)] \(source.type)")
}

// A low percentile avoids the white symbol which occupies the centre of most
// cards, while rejecting the occasional dark antialias edge from one glyph.
let percentile = images.count / 5
var background = [UInt8](repeating: 0, count: sourceSize * sourceSize)
var sample = [UInt8](repeating: 0, count: images.count)
for pixel in background.indices {
    for index in images.indices { sample[index] = images[index][pixel] }
    sample.sort()
    background[pixel] = sample[percentile]
}

let atlas = NSBitmapImageRep(
    bitmapDataPlanes: nil,
    pixelsWide: columns * cellSize,
    pixelsHigh: rows * cellSize,
    bitsPerSample: 8,
    samplesPerPixel: 4,
    hasAlpha: true,
    isPlanar: false,
    colorSpaceName: .deviceRGB,
    bytesPerRow: 0,
    bitsPerPixel: 0
)!

// Four source pixels become one atlas pixel. Background differences below ten
// levels are compression noise; a seventy-level lift is fully opaque. Averaging
// after extraction keeps the game's thin weapon lines antialiased at map scale.
for (index, image) in images.enumerated() {
    let cellX = (index % columns) * cellSize
    let cellY = (index / columns) * cellSize
    for y in 0..<cellSize {
        for x in 0..<cellSize {
            var alpha = 0.0
            for oy in 0..<4 {
                for ox in 0..<4 {
                    let pixel = (y * 4 + oy) * sourceSize + x * 4 + ox
                    let lift = max(0, Int(image[pixel]) - Int(background[pixel]) - 10)
                    alpha += min(1.0, Double(lift) / 70.0)
                }
            }
            alpha /= 16
            atlas.setColor(NSColor(deviceRed: 1, green: 1, blue: 1, alpha: alpha),
                           atX: cellX + x, y: cellY + y)
        }
    }
}

let atlasURL = URL(fileURLWithPath: CommandLine.arguments[1])
let manifestURL = URL(fileURLWithPath: CommandLine.arguments[2])
try manager.createDirectory(at: atlasURL.deletingLastPathComponent(), withIntermediateDirectories: true)
try atlas.representation(using: .png, properties: [:])!.write(to: atlasURL, options: .atomic)

let manifestUnits = units.enumerated().map { index, source in
    [
        "type": source.type,
        "index": index,
        "source_page": "https://civilization.fandom.com/wiki/File:" +
            source.title.replacingOccurrences(of: " ", with: "_"),
    ] as [String: Any]
}
let manifest: [String: Any] = [
    "description": "Civilization VI unit glyphs used by the CIVVIS strategic map",
    "cell_size": cellSize,
    "columns": columns,
    "copyright": "Civilization VI unit artwork is owned by Firaxis Games and 2K; no ownership is claimed here.",
    "source": "Civilization Wiki Unit Civilopedia icons (Civ6)",
    "units": manifestUnits,
]
let manifestData = try JSONSerialization.data(withJSONObject: manifest, options: [.prettyPrinted, .sortedKeys])
try manifestData.write(to: manifestURL, options: .atomic)
print("wrote \(atlasURL.path) and \(manifestURL.path)")
