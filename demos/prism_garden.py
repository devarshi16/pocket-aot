"""PRISM GARDEN: a colorful, interactive pocket aquarium for Pico-LCD-1.3."""
from machine import Pin, PWM
from waveshare_lcd_driver import LCD_1inch3
import framebuf
import math
import random
import time
import gc
import _thread


def rgb(r, g, b):
    v = ((int(r) & 248) << 8) | ((int(g) & 252) << 3) | (int(b) >> 3)
    return ((v & 255) << 8) | (v >> 8)


WHITE = rgb(245, 249, 255)
DARK = rgb(10, 10, 35)
GOLD = rgb(255, 222, 110)
PINK = rgb(255, 100, 200)
MINT = rgb(106, 255, 213)
INKS = ((255, 100, 176), (255, 196, 70), (90, 245, 211), (170, 132, 255))
RAINBOW = (rgb(255, 86, 155), rgb(255, 166, 88), rgb(255, 237, 118),
           rgb(98, 255, 171), rgb(87, 224, 255), rgb(170, 130, 255))
THEMES = (('NEON REEF', (9, 66, 117), (9, 14, 57)),
          ('SUNSET LAGOON', (107, 26, 103), (24, 12, 65)),
          ('EMERALD DREAM', (7, 82, 87), (10, 20, 62)))
PINS = (15, 17, 19, 21, 2, 18, 16, 20, 3)
A, B, X, Y, UP, DOWN, LEFT, RIGHT, CLICK = (1 << i for i in range(9))
RING = [(math.cos(i * math.pi / 12), math.sin(i * math.pi / 12)) for i in range(24)]


def bound(v, low, high):
    return max(low, min(high, v))


def ellipse(d, cx, cy, rx, ry, ink):
    for dy in range(-ry, ry + 1):
        dx = int(rx * math.sqrt(max(0, 1 - dy * dy / (ry * ry))))
        d.hline(cx - dx, cy + dy, dx * 2 + 1, ink)


def triangle(d, ax, ay, bx, by, cx, cy, ink):
    points = sorted(((ax, ay), (bx, by), (cx, cy)), key=lambda p: p[1])
    (ax, ay), (bx, by), (cx, cy) = points
    for y in range(ay, cy + 1):
        left = ax + (cx - ax) * (y - ay) / max(1, cy - ay)
        if y < by:
            right = ax + (bx - ax) * (y - ay) / max(1, by - ay)
        else:
            right = bx + (cx - bx) * (y - by) / max(1, cy - by)
        lo, hi = int(min(left, right)), int(max(left, right))
        d.hline(lo, y, hi - lo + 1, ink)


def ring(d, cx, cy, radius, ink):
    for i in range(24):
        x, y = RING[i]
        xx, yy = RING[(i + 1) % 24]
        d.line(int(cx + x * radius), int(cy + y * radius),
               int(cx + xx * radius), int(cy + yy * radius), ink)


class Controls:
    def __init__(self):
        self.pins = [Pin(p, Pin.IN, Pin.PULL_UP) for p in PINS]
        self.lock = _thread.allocate_lock()
        self.held = 0
        self.edges = 0
        self.running = True
        _thread.start_new_thread(self.sample, ())

    def sample(self):
        raw = [1] * 9
        stable = [1] * 9
        times = [time.ticks_ms()] * 9
        while self.running:
            now = time.ticks_ms()
            for i in range(9):
                value = self.pins[i].value()
                if value != raw[i]:
                    raw[i] = value
                    times[i] = now
                if value != stable[i] and time.ticks_diff(now, times[i]) >= 20:
                    stable[i] = value
                    bit = 1 << i
                    with self.lock:
                        if value == 0:
                            self.held |= bit
                            self.edges |= bit
                        else:
                            self.held &= ~bit
            time.sleep_ms(2)

    def read(self):
        with self.lock:
            result = self.held, self.edges
            self.edges = 0
        return result


class Garden:
    def __init__(self):
        self.light = PWM(Pin(13))
        self.light.freq(1000)
        self.light.duty_u16(45000)
        self.d = LCD_1inch3()
        self.t = 0.0
        self.idle = 0.0
        self.theme = 0
        self.jellies = False
        self.cursor = [120.0, 104.0]
        self.food = []
        self.particles = []
        self.waves = []
        self.eaten = 0
        self.message = 'A LITTLE OCEAN, JUST FOR YOU'
        self.message_t = 4.0
        self.feed_timer = 0.0
        self.creatures = []
        self.bubbles = [[random.uniform(5, 235), random.uniform(25, 215),
                         random.uniform(7, 17), random.random() * 6.28] for _ in range(9)]
        self.dust = [(random.randrange(240), random.randrange(30, 210), random.random())
                     for _ in range(25)]
        self.gradients()
        self.build_sprites()
        self.populate()

    def gradients(self):
        _, top, bottom = THEMES[self.theme]
        self.gradient = [rgb(*(top[j] + (bottom[j] - top[j]) * i / 59 for j in range(3)))
                         for i in range(60)]

    def say(self, text):
        self.message = text
        self.message_t = 2.5

    def build_sprites(self):
        self.sprites = []
        self.sprite_buffers = []
        gc.collect()
        count = 3 if self.jellies else 8
        for i in range(count):
            w, h = (48, 28) if self.jellies else (40, 24)
            buf = bytearray(w * h * 2)
            d = framebuf.FrameBuffer(buf, w, h, framebuf.RGB565)
            base = INKS[(i + self.theme) % 4] if self.jellies else INKS[(i // 2 + self.theme) % 4]
            bright = rgb(*base)
            shadow = rgb(*(v * .6 for v in base))
            glow = rgb(*(v * .27 for v in base))
            d.fill(0)
            if self.jellies:
                ellipse(d, 24, 16, 23, 12, glow)
                for yy in range(4, 23):
                    span = int(19 * math.sqrt(max(0, 1 - ((yy - 20) / 17) ** 2)))
                    shade = .58 + .4 * (23 - yy) / 19
                    d.hline(24 - span, yy, span * 2 + 1, rgb(*(v * shade for v in base)))
                d.hline(5, 22, 39, bright)
                d.hline(9, 23, 31, bright)
                for x in (11, 18, 25, 32, 39):
                    d.fill_rect(x, 24, 3, 2, bright)
                d.fill_rect(13, 10, 6, 2, WHITE)
                for x in (18, 29):
                    d.fill_rect(x, 16, 4, 4, DARK)
                    d.pixel(x + 1, 16, WHITE)
                d.hline(23, 21, 3, PINK)
            else:
                right = bool(i % 2)
                ellipse(d, 20, 12, 18, 10, glow)
                if right:
                    triangle(d, 11, 12, 2, 5, 2, 20, shadow)
                else:
                    triangle(d, 29, 12, 38, 5, 38, 20, shadow)
                ellipse(d, 20, 12, 13, 8, bright)
                ellipse(d, 20, 15, 10, 4, shadow)
                d.hline(14, 7, 11, WHITE)
                triangle(d, 15, 6, 22, 1, 25, 7, bright)
                triangle(d, 17, 17, 21, 22, 26, 17, bright)
                ex = 27 if right else 10
                ellipse(d, ex, 11, 3, 4, WHITE)
                d.fill_rect(ex + (0 if right else -1), 10, 2, 3, DARK)
                d.pixel(ex + (1 if right else -1), 10, WHITE)
                d.vline(18, 10, 7, shadow)
                d.vline(22, 10, 6, shadow)
            self.sprite_buffers.append(buf)
            self.sprites.append(d)

    def populate(self):
        self.creatures = []
        for i in range(4 if self.jellies else 7):
            self.spawn()

    def spawn(self):
        if len(self.creatures) >= (6 if self.jellies else 10):
            return
        self.creatures.append([random.uniform(35, 205), random.uniform(60, 168),
                               random.choice((-1, 1)) * random.uniform(8, 18),
                               random.uniform(-3, 3), random.random() * 6.28,
                               len(self.creatures) % (3 if self.jellies else 4)])

    def bloom(self, rainbow=False):
        x, y = self.cursor
        self.waves = [[x, y, 0.0, rainbow]]
        self.particles = [[x, y, math.cos(i * math.pi / 12) * random.uniform(18, 48),
                           math.sin(i * math.pi / 12) * random.uniform(18, 48),
                           random.uniform(1, 2), i % 6] for i in range(24)]

    def update(self, dt, held=0, edges=0):
        self.t += dt
        self.idle = 0 if held or edges else self.idle + dt
        self.message_t = max(0, self.message_t - dt)
        self.feed_timer = max(0, self.feed_timer - dt)
        if edges & X:
            self.theme = (self.theme + 1) % 3
            self.gradients()
            self.build_sprites()
            self.say(THEMES[self.theme][0])
            print('PRISM: theme', THEMES[self.theme][0])
        if edges & Y:
            self.jellies = not self.jellies
            self.build_sprites()
            self.populate()
            self.say('JELLYFISH BALLET' if self.jellies else 'THE RAINBOW SCHOOL')
            print('PRISM: habitat', 'jellyfish' if self.jellies else 'fish')
        if edges & B:
            self.bloom(True)
            self.say('MAKE A LITTLE MAGIC')
        if edges & CLICK:
            self.bloom()
            self.spawn()
            self.say('HELLO, LITTLE FRIEND')
        self.cursor[0] = bound(self.cursor[0] + (bool(held & RIGHT) - bool(held & LEFT)) * dt * 80, 22, 218)
        self.cursor[1] = bound(self.cursor[1] + (bool(held & DOWN) - bool(held & UP)) * dt * 70, 40, 190)
        if self.idle > 10:
            self.cursor[0] = 120 + math.sin(self.t * .35) * 65
            self.cursor[1] = 111 + math.cos(self.t * .29) * 42
        if (edges | held) & A and self.feed_timer <= 0:
            self.feed_timer = .5
            for i in range(4):
                if len(self.food) < 16:
                    self.food.append([self.cursor[0] + random.uniform(-10, 10),
                                      self.cursor[1] + random.uniform(-7, 7), 9.0])
            self.say('DINNER IS SERVED')
        for food in self.food:
            food[1] += dt * 4
            food[2] -= dt
        for creature in self.creatures:
            x, y, vx, vy, phase, ink = creature
            desired_x = math.sin(self.t * .45 + phase) * 13
            desired_y = math.cos(self.t * .6 + phase) * 5
            target = None
            if self.food:
                target = min(self.food, key=lambda f: (f[0] - x) ** 2 + (f[1] - y) ** 2)
            elif self.idle < 8:
                target = self.cursor
            if target is not None:
                dx, dy = target[0] - x, target[1] - y
                distance = math.sqrt(dx * dx + dy * dy)
                if distance > 24 or self.food:
                    speed = 30 if self.food else 14
                    desired_x, desired_y = dx / max(1, distance) * speed, dy / max(1, distance) * speed
                if self.food and distance < 13:
                    target[2] = 0
                    self.food.remove(target)
                    self.eaten += 1
            if self.jellies:
                desired_y += math.sin(self.t * 2 + phase) * 7
                desired_x *= .65
            if x < 30:
                desired_x = abs(desired_x) + 8
            if x > 210:
                desired_x = -abs(desired_x) - 8
            if y < 56:
                desired_y = abs(desired_y) + 6
            if y > 174:
                desired_y = -abs(desired_y) - 6
            blend = min(1, dt * 1.7)
            vx += (desired_x - vx) * blend
            vy += (desired_y - vy) * blend
            creature[0] = bound(x + vx * dt, 23, 217)
            creature[1] = bound(y + vy * dt, 45, 185)
            creature[2], creature[3] = vx, vy
        self.food = [f for f in self.food if f[2] > 0 and f[1] < 204]
        for p in self.particles:
            p[0] += p[2] * dt
            p[1] += p[3] * dt
            p[4] -= dt
        self.particles = [p for p in self.particles if p[4] > 0]
        for w in self.waves:
            w[2] += dt
        self.waves = [w for w in self.waves if w[2] < 2.4]
        for b in self.bubbles:
            b[1] -= dt * b[2]
            if b[1] < 27:
                b[1], b[0] = 215.0, random.uniform(8, 232)

    def water(self):
        d = self.d
        for i, ink in enumerate(self.gradient):
            d.fill_rect(0, i * 4, 240, 4, ink)
        # Shimmering broken caustics below the surface.
        caustic = rgb(40, 118, 160) if self.theme != 1 else rgb(149, 58, 147)
        for j in range(4):
            for i in range(5):
                x = int((i * 59 + math.sin(self.t * .6 + j + i) * 20 + j * 23) % 260) - 10
                y = 34 + j * 10 + int(math.sin(self.t + i) * 3)
                d.hline(x, y, 15 + i * 2, caustic)
        for x, y, seed in self.dust:
            yy = int(28 + (y - self.t * (1 + seed * 2)) % 181)
            d.pixel(x, yy, RAINBOW[int(seed * 6) % 6])

    def reef(self):
        d = self.d
        for i, x in enumerate((10, 44, 83, 137, 185, 226)):
            height = 24 + (i * 11) % 36
            previous_x, previous_y = x, 219
            ink = RAINBOW[(i + self.theme * 2) % 6]
            for j in range(1, 6):
                y = 219 - j * height // 5
                xx = x + int(math.sin(self.t * .8 + i + j * .5) * j * 1.4)
                d.line(previous_x, previous_y, xx, y, ink)
                d.line(previous_x + 1, previous_y, xx + 1, y, ink)
                if j < 5:
                    side = -1 if j % 2 else 1
                    end = xx + side * (9 - j)
                    d.line(xx, y, end, y - 7, ink)
                    d.pixel(end, y - 8, WHITE)
                previous_x, previous_y = xx, y
            ellipse(d, x, 218, 12, 4, rgb(34 + i * 6, 29, 83 + i * 8))
        for i in range(16):
            x = (i * 43) % 240
            y = 211 + (i * 7) % 11
            d.hline(x, y, 3, RAINBOW[i % 6])

    def animals(self):
        d = self.d
        for x, y, vx, vy, phase, ink in self.creatures:
            bob = math.sin(self.t * 2 + phase) * 2
            if self.jellies:
                for leg in range(5):
                    px = x - 15 + leg * 7
                    py = y + 9 + bob
                    for segment in range(1, 5):
                        xx = x - 15 + leg * 7 + math.sin(self.t * 2.7 + phase + segment * .8 + leg) * segment * 1.8
                        yy = y + 9 + segment * 6 + bob
                        color = RAINBOW[(ink + leg + self.theme) % 6]
                        d.line(int(px), int(py), int(xx), int(yy), color)
                        px, py = xx, yy
                d.blit(self.sprites[ink % 3], int(x) - 24, int(y + bob) - 14, 0)
            else:
                index = ink * 2 + int(vx > 0)
                d.blit(self.sprites[index], int(x) - 20, int(y + bob) - 12, 0)
                # A flickering little pectoral fin keeps each fish alive.
                d.line(int(x), int(y + bob + 1), int(x - 3),
                       int(y + bob + 5 + math.sin(self.t * 9 + phase) * 2), WHITE)

    def effects(self):
        d = self.d
        for b in self.bubbles:
            x = int(b[0] + math.sin(self.t + b[3]) * 4)
            y = int(b[1])
            d.rect(x, y, 4, 4, rgb(58, 133, 176))
            d.pixel(x + 1, y, WHITE)
        for f in self.food:
            d.fill_rect(int(f[0]), int(f[1]), 3, 3, GOLD)
        for wave in self.waves:
            for i in range(4 if wave[3] else 2):
                r = wave[2] * 46 - i * 10
                if r > 1:
                    ring(d, wave[0], wave[1], r, RAINBOW[(i + int(self.t * 3)) % 6] if wave[3] else MINT)
        for p in self.particles:
            x, y = int(p[0]), int(p[1])
            if 0 < x < 238 and 26 < y < 220:
                d.fill_rect(x, y, 2, 2, RAINBOW[p[5]])
        x, y = (int(v) for v in self.cursor)
        # A rainbow cursor: little creatures follow its light.
        for i, (dx, dy) in enumerate(RING[::4]):
            xx, yy = int(x + dx * 9), int(y + dy * 9)
            d.fill_rect(xx, yy, 2, 2, RAINBOW[(i + int(self.t * 2)) % 6])
        d.hline(x - 3, y, 7, WHITE)
        d.vline(x, y - 3, 7, WHITE)

    def draw(self):
        self.water()
        self.reef()
        self.animals()
        self.effects()
        d = self.d
        # The header occupies only a slim strip, leaving the world in view.
        d.fill_rect(0, 0, 240, 24, DARK)
        for i in range(6):
            d.fill_rect(i * 40, 22, 40, 2, RAINBOW[i])
        d.text('PRISM GARDEN', 8, 8, WHITE)
        d.text('JELLIES' if self.jellies else 'FISH', 176, 8, MINT)
        d.fill_rect(0, 224, 240, 16, DARK)
        if self.message_t:
            text = self.message
        else:
            tips = ('A:FEED B:RAINBOW X:COLOR', 'Y:FISH/JELLY CLICK:NEW FRIEND',
                    'STICK:MOVES YOUR MAGIC LIGHT', THEMES[self.theme][0])
            text = tips[int(self.t // 4) % len(tips)]
        d.text(text[:29], 4, 229, GOLD if self.message_t else WHITE)
        d.show()

    def boot(self):
        self.water()
        self.reef()
        self.animals()
        self.d.fill_rect(28, 76, 184, 69, DARK)
        for i, word in enumerate(('P R I S M', 'G A R D E N')):
            self.d.text(word, 80 if i == 0 else 72, 89 + i * 19, RAINBOW[(i + 2) % 6])
        self.d.text('A POCKET FULL OF WONDER', 32, 130, WHITE)
        self.d.show()
        time.sleep_ms(1100)


def run():
    gc.collect()
    garden = Garden()
    garden.boot()
    controls = Controls()
    print('PRISM_READY: A=feed B=rainbow X=theme Y=species CLICK=friend')
    last = time.ticks_ms()
    try:
        while True:
            now = time.ticks_ms()
            dt = bound(time.ticks_diff(now, last) / 1000, .001, .3)
            last = now
            held, edges = controls.read()
            garden.update(dt, held, edges)
            garden.draw()
            time.sleep_ms(2)
    finally:
        controls.running = False
        time.sleep_ms(50)


if __name__ == '__main__':
    run()
