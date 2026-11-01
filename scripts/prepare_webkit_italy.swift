// macOS-only, offline asset preparation. Runtime consumes pixels + linear-P3 metadata.
import Foundation
import CoreGraphics
import ImageIO

let directory = CommandLine.arguments[1]
for name in ["Italy-sRGB", "Italy-P3", "Italy-oog"] {
    let url = URL(fileURLWithPath: directory).appendingPathComponent(name + ".jpg")
    guard let source = CGImageSourceCreateWithURL(url as CFURL, nil),
          var image = CGImageSourceCreateImageAtIndex(source, 0, nil),
          let destination = CGColorSpace(name: CGColorSpace.extendedLinearDisplayP3) else { fatalError("Decode \(name)") }
    if name == "Italy-sRGB" {
        // The source webpage's sRGB JPEG is untagged: its documented interpretation is sRGB.
        image = image.copy(colorSpace: CGColorSpace(name: CGColorSpace.sRGB)!)!
    }
    let width = image.width, height = image.height
    var values = [Float](repeating: 0, count: width * height * 4)
    values.withUnsafeMutableBytes { bytes in
        let flags = CGBitmapInfo.floatComponents.rawValue | CGBitmapInfo.byteOrder32Little.rawValue | CGImageAlphaInfo.premultipliedLast.rawValue
        guard let context = CGContext(data: bytes.baseAddress, width: width, height: height,
                                      bitsPerComponent: 32, bytesPerRow: width * 16,
                                      space: destination, bitmapInfo: flags) else { fatalError("Context") }
        context.setRenderingIntent(.relativeColorimetric)
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
    }
    var result = Data(capacity: values.count * 2)
    for value in values {
        precondition(value.isFinite)
        var bits = Float16(value).bitPattern.littleEndian
        withUnsafeBytes(of: &bits) { result.append(contentsOf: $0) }
    }
    try result.write(to: URL(fileURLWithPath: directory).appendingPathComponent(name + ".rgba16f"))
    print("\(name): \(width)x\(height), \(result.count) bytes, source color space \(image.colorSpace?.name as String? ?? "embedded ICC")")
}
