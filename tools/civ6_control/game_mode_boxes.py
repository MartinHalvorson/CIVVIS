"""Find a visible checkbox outline near a calibrated setup coordinate."""

import math


def checkbox_center(image, center, scale):
    radius = math.ceil(24 * scale)
    minimum = max(6, round(12 * scale))
    maximum = max(minimum, round(22 * scale))
    left = max(0, round(center[0]) - radius - maximum)
    right = min(image.width, round(center[0]) + radius + maximum)
    top = max(0, round(center[1]) - radius - maximum)
    bottom = min(image.height, round(center[1]) + radius + maximum)
    gold = set()
    for vertical in range(top, bottom):
        for horizontal in range(left, right):
            red, green, blue = image.getpixel((horizontal, vertical))[:3]
            if red > 75 and green > 55 and red > blue * 1.3 and green > blue * 1.2:
                gold.add((horizontal, vertical))
    candidates = []
    for vertical in range(top, bottom):
        horizontal = left
        while horizontal < right:
            if (horizontal, vertical) not in gold:
                horizontal += 1
                continue
            start = horizontal
            while horizontal < right and (horizontal, vertical) in gold:
                horizontal += 1
            width = horizontal - start
            if not minimum <= width <= maximum:
                continue
            lower = vertical + width - 1
            if lower >= bottom:
                continue
            edges = [
                sum((column, lower) in gold for column in range(start, horizontal)),
                sum((start, row) in gold for row in range(vertical, lower + 1)),
                sum((horizontal - 1, row) in gold for row in range(vertical, lower + 1)),
            ]
            if min(edges) < width * 0.7:
                continue
            candidate = ((start + horizontal - 1) / 2, (vertical + lower) / 2)
            distance = math.dist(candidate, center)
            if distance <= radius:
                candidates.append((distance, candidate, width))
    if not candidates:
        return None
    _, center, width = min(candidates)
    return center[0], center[1], width
