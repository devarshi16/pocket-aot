"""NOMAD-01: a self-contained pocket starship for Waveshare Pico-LCD-1.3."""
from machine import Pin, PWM
from waveshare_lcd_driver import LCD_1inch3
import framebuf
import math
import random
import time
import gc
import _thread


def color(r, g, b):
    n = ((r & 248) << 8) | ((g & 252) << 3) | (b >> 3)
    return ((n & 255) << 8) | (n >> 8)


BLACK = color(2, 5, 12)
PANEL = color(6, 18, 29)
DIM = color(23, 58, 75)
CYAN = color(80, 225, 245)
WHITE = color(224, 245, 248)
GOLD = color(255, 186, 75)
RED = color(255, 85, 77)
GREEN = color(105, 237, 164)
FAINT = color(60, 105, 126)
PALETTES = ((72, 176, 202), (225, 138, 75), (150, 113, 210),
            (80, 183, 132), (204, 111, 153), (170, 190, 207))
WORLDS = ('KEPLER VEIL', 'EMBER REACH', 'VIOLET SEA', 'EDEN PRIME',
          'ROSE NEBULA', 'GHOST HARBOR')
BIOMES = ('OCEAN WORLD', 'VOLCANIC WORLD', 'CRYSTAL WORLD', 'FOREST WORLD',
          'TWILIGHT WORLD', 'FROZEN WORLD')
SIGNALS = ('A city beneath the ice.', 'Something answers back.',
           'An ancient beacon wakes.', 'Life among the clouds.',
           'A ship with no crew.', 'A song in the static.')
PINS = (15, 17, 19, 21, 2, 18, 16, 20, 3)
A, B, X, Y = 1, 2, 4, 8
UP, DOWN, LEFT, RIGHT, CENTRE = 16, 32, 64, 128, 256


def clamp(n, lo, hi):
    return max(lo, min(hi, n))


def disc(d, x, y, r, ink):
    r = max(1, int(r))
    for dy in range(-r, r + 1):
        dx = int(math.sqrt(max(0, r * r - dy * dy)))
        d.hline(int(x) - dx, int(y) + dy, dx * 2 + 1, ink)


def circle(d, x, y, r, ink):
    x, y, r = int(x), int(y), int(r)
    px, py, err = r, 0, 1 - r
    while px >= py:
        for dx, dy in ((px, py), (py, px), (-py, px), (-px, py),
                       (-px, -py), (-py, -px), (py, -px), (px, -py)):
            d.pixel(x + dx, y + dy, ink)
        py += 1
        if err < 0:
            err += 2 * py + 1
        else:
            px -= 1
            err += 2 * (py - px) + 1


def large(d, text, x, y, ink, scale=2):
    buf = bytearray(128)
    font = framebuf.FrameBuffer(buf, 128, 8, framebuf.MONO_HLSB)
    font.text(text, 0, 0, 1)
    for yy in range(8):
        for xx in range(min(128, len(text) * 8)):
            if font.pixel(xx, yy):
                d.fill_rect(x + xx * scale, y + yy * scale, scale, scale, ink)


class Controls:
    """Core 1 debounces and latches edges even during a display transfer."""
    def __init__(self):
        self.pins = [Pin(g, Pin.IN, Pin.PULL_UP) for g in PINS]
        self.lock = _thread.allocate_lock()
        self.mask = 0
        self.edges = 0
        self.running = True
        _thread.start_new_thread(self.sample, ())

    def sample(self):
        raw = [1] * 9
        stable = [1] * 9
        changed = [time.ticks_ms()] * 9
        while self.running:
            now = time.ticks_ms()
            for i in range(9):
                value = self.pins[i].value()
                if value != raw[i]:
                    raw[i] = value
                    changed[i] = now
                if value != stable[i] and time.ticks_diff(now, changed[i]) >= 20:
                    stable[i] = value
                    bit = 1 << i
                    with self.lock:
                        if value == 0:
                            self.mask |= bit
                            self.edges |= bit
                        else:
                            self.mask &= ~bit
            time.sleep_ms(2)

    def read(self):
        with self.lock:
            result = (self.mask, self.edges)
            self.edges = 0
        return result


class Ship:
    def __init__(self):
        self.light = PWM(Pin(13))
        self.light.freq(1000)
        self.light.duty_u16(40000)
        self.d = LCD_1inch3()
        self.t = 0.0
        self.idle = 0.0
        self.mode = 0
        self.world = 0
        self.selected = 1
        self.discovered = 0
        self.scan = 0.0
        self.scan_done = False
        self.score = 0
        self.hull = 100.0
        self.energy = 100.0
        self.shield = 0.0
        self.jump = 0.0
        self.cooldown = 0.0
        self.shot = 0.0
        self.damage = 0.0
        self.fire_timer = 0.0
        self.aim_x = 0.0
        self.aim_y = 0.0
        self.notice = 'WELCOME, COMMANDER'
        self.notice_t = 5.0
        self.signal_timer = 18.0
        self.frames = 0
        self.planet_key = None
        self.planet_image = None
        self.planet_buffer = None
        self.stars = [[random.uniform(-140, 140), random.uniform(-100, 100),
                       random.uniform(3, 190)] for _ in range(65)]
        self.rocks = []
        for _ in range(6):
            self.rocks.append([random.uniform(-110, 110), random.uniform(-65, 65),
                               random.uniform(110, 420), random.random() * 6.28])
        self.particles = []
        self.nodes = [(random.uniform(-70, 70), random.uniform(-52, 52),
                       random.uniform(-55, 55)) for _ in range(6)]

    def say(self, text, duration=2.5):
        self.notice = text
        self.notice_t = duration

    def reset_rock(self, rock):
        rock[0] = random.uniform(-110, 110)
        rock[1] = random.uniform(-65, 65)
        rock[2] = random.uniform(260, 440)

    def project(self, rock):
        z = max(5, rock[2])
        return (int(120 + rock[0] * 135 / z),
                int(95 + rock[1] * 135 / z), max(2, int(9 * 135 / z)))

    def begin_jump(self):
        if self.jump or self.cooldown:
            self.say('JUMP DRIVE RECHARGING')
            return
        self.jump = 2.8
        self.mode = 0
        self.say('HYPERDRIVE ENGAGED', 2.8)
        print('NOMAD: hyperspace to', WORLDS[self.selected])

    def fire(self):
        self.shot = 0.16
        self.fire_timer = 0.28
        hit = None
        for rock in self.rocks:
            x, y, r = self.project(rock)
            if 24 < y < 170 and abs(x - 120) < r + 23 and abs(y - 96) < r + 23:
                if hit is None or rock[2] < hit[2]:
                    hit = rock
        if hit is not None:
            x, y, _ = self.project(hit)
            self.particles = [[float(x), float(y), random.uniform(-45, 45),
                               random.uniform(-45, 45), random.uniform(.4, .9)]
                              for _ in range(14)]
            self.score += 100
            self.say('ASTEROID DESTROYED +100', 1.5)
            self.reset_rock(hit)

    def update(self, dt, held=0, edges=0):
        self.t += dt
        self.idle = 0.0 if held or edges else self.idle + dt
        self.notice_t = max(0, self.notice_t - dt)
        self.signal_timer -= dt
        if self.signal_timer <= 0 and self.mode == 0 and not self.jump:
            self.say('UNKNOWN SIGNAL DETECTED', 4)
            self.signal_timer = random.uniform(25, 45)
        self.shot = max(0, self.shot - dt)
        self.damage = max(0, self.damage - dt)
        self.fire_timer = max(0, self.fire_timer - dt)
        self.cooldown = max(0, self.cooldown - dt)
        if edges & X:
            self.mode = 0 if self.mode == 1 else 1
            self.say('STAR CHART / SELECT + JUMP' if self.mode else 'FLIGHT CONTROL')
        if edges & Y:
            self.mode = 0 if self.mode == 2 else 2
            self.say('PLANETARY SCANNER' if self.mode else 'FLIGHT CONTROL')
        if edges & CENTRE:
            self.begin_jump()
        if self.mode == 1 and not self.jump:
            if edges & (RIGHT | DOWN):
                self.selected = (self.selected + 1) % 6
            if edges & (LEFT | UP):
                self.selected = (self.selected - 1) % 6
            if edges & A:
                self.begin_jump()
        steer_x = bool(held & RIGHT) - bool(held & LEFT)
        steer_y = bool(held & DOWN) - bool(held & UP)
        autopilot = self.idle > 12
        if autopilot:
            steer_x = math.sin(self.t * .34) * .45
            steer_y = math.cos(self.t * .27) * .3
        self.aim_x += (steer_x * 18 - self.aim_x) * min(1, dt * 5)
        self.aim_y += (steer_y * 14 - self.aim_y) * min(1, dt * 5)
        if held & B and self.energy > 1:
            self.shield = min(1.0, self.shield + dt * 5)
            self.energy = max(0, self.energy - dt * 15)
            self.hull = min(100, self.hull + dt * 1.5)
        else:
            self.shield = max(0, self.shield - dt * 2)
            self.energy = min(100, self.energy + dt * 9)
        if (held | edges) & A and self.mode == 0 and not self.fire_timer and not self.jump:
            self.fire()
        if self.jump:
            self.jump = max(0, self.jump - dt)
            if not self.jump:
                self.world = self.selected
                self.selected = (self.world + 1) % 6
                self.scan = 0
                self.scan_done = False
                self.cooldown = 4
                self.say('ARRIVED: ' + WORLDS[self.world], 4)
                for rock in self.rocks:
                    self.reset_rock(rock)
        speed = 220 if self.jump else 30
        for star in self.stars:
            star[2] -= dt * speed
            star[0] -= steer_x * dt * 12
            star[1] -= steer_y * dt * 10
            if star[2] < 2 or abs(star[0]) > 170 or abs(star[1]) > 130:
                star[:] = [random.uniform(-140, 140), random.uniform(-100, 100), 190.0]
        if self.mode == 0 and not self.jump:
            for rock in self.rocks:
                rock[2] -= dt * 15
                rock[0] -= steer_x * dt * 10
                rock[1] -= steer_y * dt * 8
                rock[3] += dt * .5
                if rock[2] < 12:
                    if abs(rock[0]) < 19 and abs(rock[1]) < 15:
                        self.damage = .4
                        if self.shield > .3:
                            self.say('SHIELDS ABSORBED IMPACT')
                        else:
                            self.hull = max(0, self.hull - 18)
                            self.say('IMPACT / HOLD B TO REPAIR')
                    self.reset_rock(rock)
            if self.hull <= 0:
                self.hull = 65
                self.cooldown = 0
                self.begin_jump()
                self.say('EMERGENCY JUMP', 3)
        for p in self.particles:
            p[0] += p[2] * dt
            p[1] += p[3] * dt
            p[4] -= dt
        self.particles = [p for p in self.particles if p[4] > 0]
        if self.mode == 2 and not self.jump:
            self.scan = min(1, self.scan + dt / 5)
            if self.scan >= 1 and not self.scan_done:
                self.scan_done = True
                self.discovered |= 1 << self.world
                self.score += 250
                self.say('DISCOVERY LOGGED +250', 3)
                print('NOMAD: scanned', WORLDS[self.world])

    def planet(self, cx, cy, radius):
        # The lit surface is static within a sector. Cache it so animation time
        # goes to the moving stars rather than repainting every shaded scanline.
        r = int(radius)
        key = (self.world, r)
        if key != self.planet_key:
            self.planet_image = None
            self.planet_buffer = None
            gc.collect()
            size = r * 2 + 6
            self.planet_buffer = bytearray(size * size * 2)
            self.planet_image = framebuf.FrameBuffer(self.planet_buffer, size, size, framebuf.RGB565)
            self.planet_image.fill(BLACK)
            display = self.d
            self.d = self.planet_image
            self.paint_planet(r + 3, r + 3, r)
            self.d = display
            self.planet_key = key
        self.d.blit(self.planet_image, int(cx) - r - 3, int(cy) - r - 3, BLACK)

    def paint_planet(self, cx, cy, radius):
        d = self.d
        base = PALETTES[self.world]
        r = int(radius)
        for dy in range(-r, r + 1):
            half = int(math.sqrt(max(0, r * r - dy * dy)))
            if not half:
                continue
            light = .66 + .24 * math.cos(dy * 2 / r)
            lit = color(int(base[0] * light), int(base[1] * light), int(base[2] * light))
            dark = color(int(base[0] * .16), int(base[1] * .19), int(base[2] * .25))
            left = int(cx) - half
            width = 2 * half + 1
            d.hline(left, int(cy) + dy, width, dark)
            terminator = int(cx - half * .28 + dy * .15)
            d.hline(left, int(cy) + dy, max(0, terminator - left), lit)
            if (dy + r) % 12 < 2 and half > 8:
                d.hline(left + 4, int(cy) + dy, max(1, (terminator - left) // 2), FAINT)
        circle(d, cx, cy, r + 2, DIM)
        circle(d, cx, cy, r + 1, FAINT)

    def backdrop(self):
        d = self.d
        for sx, sy, z in self.stars:
            x, y = int(120 + sx * 130 / z), int(95 + sy * 130 / z)
            if 1 < x < 238 and 25 < y < 171:
                ink = WHITE if z < 40 else (CYAN if z < 90 else FAINT)
                if self.jump:
                    k = min(.55, 18 / z)
                    xx = int(x + (x - 120) * k)
                    yy = int(y + (y - 95) * k)
                    d.line(x, y, clamp(xx, 0, 239), clamp(yy, 25, 170), ink)
                else:
                    d.pixel(x, y, ink)
                    if z < 25:
                        d.pixel(x + 1, y, ink)

    def flight(self):
        d = self.d
        if not self.jump:
            self.planet(186 + int(self.aim_x * .3), 72 + int(self.aim_y * .3), 29)
            for rock in sorted(self.rocks, key=lambda r: -r[2]):
                x, y, r = self.project(rock)
                if not (r < x < 240 - r and 25 + r < y < 171 - r):
                    continue
                pts = []
                for i in range(7):
                    angle = rock[3] + i * 6.283 / 7
                    rr = r * (1 if i % 2 else .72)
                    pts.append((int(x + math.cos(angle) * rr), int(y + math.sin(angle) * rr)))
                for i in range(7):
                    d.line(pts[i][0], pts[i][1], pts[(i + 1) % 7][0], pts[(i + 1) % 7][1], GOLD)
                d.line(pts[0][0], pts[0][1], pts[3][0], pts[3][1], DIM)
                d.line(pts[3][0], pts[3][1], pts[5][0], pts[5][1], DIM)
        for p in self.particles:
            x, y = int(p[0]), int(p[1])
            if 0 < x < 238 and 25 < y < 169:
                d.fill_rect(x, y, 2, 2, GOLD)
        if self.shot:
            d.line(22, 170, 118, 96, RED)
            d.line(218, 170, 122, 96, RED)
            circle(d, 120, 96, 4, WHITE)
        cx, cy = 120, 96
        for sign in (-1, 1):
            d.line(cx + sign * 7, cy, cx + sign * 14, cy, CYAN)
            d.line(cx, cy + sign * 7, cx, cy + sign * 11, CYAN)
        d.pixel(cx, cy, WHITE)
        # The cockpit canopy and a gently banking horizon.
        for sign in (-1, 1):
            d.line(120 + sign * 84, 171, 120 + sign * 114, 146, DIM)
            d.line(120 + sign * 114, 146, 120 + sign * 119, 60, DIM)
        d.line(96, 155 + int(self.aim_x * .3), 144, 155 - int(self.aim_x * .3), FAINT)
        if self.shield > .1:
            for radius in (68, 73):
                for i in range(16):
                    a = i * math.pi / 8 + self.t * .3
                    x, y = int(120 + math.cos(a) * radius), int(96 + math.sin(a) * radius)
                    d.pixel(x, y, CYAN)
                    d.pixel(x + 1, y, CYAN)
            d.text('SHIELD ACTIVE', 68, 148, CYAN)
        if self.jump:
            d.fill_rect(54, 73, 132, 40, BLACK)
            d.text('H Y P E R S P A C E', 48, 80, WHITE)
            d.text('FOLDING SPACETIME', 56, 100, CYAN)
        elif self.idle > 12:
            d.text('AUTO', 8, 31, GREEN)

    def chart(self):
        d = self.d
        d.text('LOCAL CONSTELLATION', 8, 31, CYAN)
        angle = self.t * .12
        projected = []
        for x, y, z in self.nodes:
            xx = x * math.cos(angle) + z * math.sin(angle)
            zz = -x * math.sin(angle) + z * math.cos(angle)
            scale = 180 / (220 + zz)
            projected.append((int(120 + xx * scale), int(99 + y * scale)))
        for i in range(6):
            x, y = projected[i]
            nx, ny = projected[(i + 1) % 6]
            d.line(x, y, nx, ny, DIM)
        for i, (x, y) in enumerate(projected):
            ink = GOLD if i == self.selected else (GREEN if self.discovered & (1 << i) else CYAN)
            disc(d, x, y, 3 if i != self.selected else 4, ink)
            if i == self.selected:
                circle(d, x, y, 9 + int(math.sin(self.t * 3) * 2), GOLD)
                d.text(str(i + 1), x + 12, y - 3, GOLD)
            if i == self.world:
                d.rect(x - 6, y - 6, 13, 13, WHITE)
        d.fill_rect(0, 153, 240, 20, BLACK)
        d.text(WORLDS[self.selected], 8, 154, GOLD)
        d.text('STICK:SELECT A:JUMP', 8, 165, FAINT)

    def scanner(self):
        d = self.d
        d.text('PLANETARY SURVEY', 8, 31, CYAN)
        self.planet(66, 94, 38)
        sweep = 51 + int((self.t * 35) % 85)
        d.hline(22, sweep, 89, GREEN)
        d.line(22, 48, 32, 48, GREEN)
        d.line(22, 48, 22, 58, GREEN)
        d.line(110, 139, 100, 139, GREEN)
        d.line(110, 139, 110, 129, GREEN)
        d.text('SURVEY', 126, 54, FAINT)
        d.text('%03d%%' % (self.scan * 100), 126, 70, GREEN)
        d.rect(126, 86, 100, 6, DIM)
        d.fill_rect(127, 87, int(self.scan * 98), 4, GREEN)
        if self.scan > .3:
            d.text('ATMOSPHERE', 126, 103, FAINT)
            d.text(('BREATHABLE', 'CORROSIVE', 'IONIZED', 'OXYGEN', 'METHANE', 'THIN')[self.world], 126, 116, WHITE)
        d.text(BIOMES[self.world], 8, 146, CYAN)
        if self.scan_done:
            d.text(SIGNALS[self.world], 8, 160, GOLD)
        else:
            d.text('DECODING SIGNAL' + '.' * (int(self.t * 3) % 4), 8, 160, GOLD)

    def hud(self):
        d = self.d
        d.fill_rect(0, 0, 240, 24, PANEL)
        d.text('NOMAD', 8, 8, CYAN)
        d.text(('FLIGHT', 'ATLAS', 'SURVEY')[self.mode], 72, 8, WHITE)
        d.text('%05d' % min(self.score, 99999), 192, 8, GOLD)
        d.hline(0, 23, 240, DIM)
        d.fill_rect(0, 174, 240, 66, PANEL)
        d.hline(0, 174, 240, CYAN if not self.damage else RED)
        # Miniature radar.
        circle(d, 28, 198, 17, DIM)
        circle(d, 28, 198, 8, DIM)
        d.hline(10, 198, 37, DIM)
        d.vline(28, 180, 37, DIM)
        a = self.t * 1.8
        d.line(28, 198, int(28 + math.cos(a) * 16), int(198 + math.sin(a) * 16), CYAN)
        for rock in self.rocks:
            rx = int(rock[0] * .13)
            ry = int((rock[2] / 440) * 25 - 12)
            d.pixel(28 + rx, 198 + ry, GOLD)
        d.text('HULL', 58, 185, FAINT)
        d.text('CORE', 58, 201, FAINT)
        for yy, value, ink in ((186, self.hull, GREEN if self.hull > 35 else RED),
                               (202, self.energy, CYAN)):
            d.rect(98, yy, 69, 6, DIM)
            d.fill_rect(99, yy + 1, int(value * .67), 4, ink)
        d.text('%03d' % self.hull, 174, 185, WHITE)
        d.text('%03d' % self.energy, 174, 201, WHITE)
        # Animated engine exhaust meter.
        for i in range(4):
            height = 5 + int((math.sin(self.t * 12 + i * 1.5) + 1) * 8)
            d.fill_rect(211 + i * 5, 211 - height, 3, height, GOLD if self.jump else CYAN)
        text = self.notice if self.notice_t else WORLDS[self.world]
        d.text(text[:29], 4, 218, GOLD if self.notice_t else FAINT)
        d.text('A:FIRE B:SHLD X:MAP Y:SCAN', 4, 232, FAINT)

    def draw(self):
        d = self.d
        d.fill(BLACK)
        self.backdrop()
        if self.mode == 1 and not self.jump:
            self.chart()
        elif self.mode == 2 and not self.jump:
            self.scanner()
        else:
            self.flight()
        self.hud()
        if self.damage and int(self.t * 20) % 2:
            d.rect(0, 24, 240, 150, RED)
        d.show()
        self.frames += 1

    def boot(self):
        d = self.d
        messages = ('REACTOR ........ ONLINE', 'NAV ARRAY ...... ONLINE',
                    'SHIELDS ........ ONLINE', 'FLIGHT CORE .... ONLINE')
        for stage in range(5):
            d.fill(BLACK)
            d.hline(20, 42, 200, DIM)
            large(d, 'NOMAD-01', 24, 57, CYAN, 3)
            d.text('DEEP SPACE EXPLORER', 48, 88, FAINT)
            for i in range(min(stage, 4)):
                d.text(messages[i], 24, 117 + 16 * i, GREEN)
            d.rect(24, 194, 192, 5, DIM)
            d.fill_rect(25, 195, int(190 * stage / 4), 3, CYAN)
            d.text('CLICK STICK TO JUMP', 48, 218, WHITE)
            d.show()
            time.sleep_ms(260 if stage < 4 else 650)


def run():
    gc.collect()
    ship = Ship()
    ship.boot()
    controls = Controls()
    print('NOMAD_READY: A=fire B=shield X=map Y=scan CENTRE=jump')
    last = time.ticks_ms()
    try:
        while True:
            now = time.ticks_ms()
            dt = clamp(time.ticks_diff(now, last) / 1000, .001, .3)
            last = now
            held, edges = controls.read()
            ship.update(dt, held, edges)
            ship.draw()
            time.sleep_ms(2)
    finally:
        controls.running = False
        time.sleep_ms(50)


if __name__ == '__main__':
    run()
