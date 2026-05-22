# Unison Supersaw v1.0 - AMD Envelope

**Polyphonic Unison Supersaw with Attack-Multiply-Decay Envelope**

A bright, punchy 2-layer supersaw with one-shot AMD envelope for plucky, staccato sounds.

---

## Features no GUI

### AMD Envelope (Attack-Multiply-Decay)
- **Attack:** Fast fade-in (1-120ms)
- **Multiply:** Hold at maximum (5-200ms) - sustains the impact
- **Decay:** Exponential fade-out (10-800ms)
- **One-shot:** No sustain or release - perfect for plucks and stabs!

### Oscillators
- **2× Supersaw layers** - thinner than Hypersaw's 4×
- **Random detune algorithm** - organic, wide detune (80% default!)
- **Random stereo spread** - evolving stereo field
- **Up to 32 unison voices** per layer
- **Poly-BLEP anti-aliasing** - clean sawtooth waves

---

## Parameters

| Parameter      | Range      | Default | Description                          |
|----------------|------------|---------|--------------------------------------|
| Gain           | -36 to 0dB | -6dB    | Master output level                  |
| Attack         | 1-120ms    | 10ms    | Fade-in time                         |
| Multiply       | 5-200ms    | 50ms    | Hold at peak (longer = punchier)     |
| Decay          | 10-800ms   | 300ms   | Exponential fade-out                 |
| Unison Voices  | 1-32       | 6       | Number of unison voices per layer    |
| Detune         | 20-100%    | 80%     | Wide detune for supersaw character   |
| Spread         | 0-100%     | 80%     | Stereo spread width                  |

---

## AMD vs ADSR

| Feature        | ADSR (Hypersaw) | AMD (Supersaw)  | Why?                    |
|----------------|-----------------|-----------------|-------------------------|
| Envelope type  | Attack-D-S-R    | Attack-M-D      | One-shot plucks         |
| Note handling  | Note on/off     | Note on only    | No sustain pedal needed |
| Use case       | Pads, sustained | Plucks, stabs   | Different playing style |
| Decay curve    | Digital (95%+cut)| Exponential    | Smoother natural decay  |

---

## AMD Envelope Explained

```
Level
  1.0 ┤    ╭──────╮         
      │   ╱        ╲        
      │  ╱          ╲       
  0.5 ┤ ╱            ╲      
      │╱              ╲___  
  0.0 ┴────────────────────
       A    M      D
```

- **Attack (A):** Quadratic fade-in (smooth start)
- **Multiply (M):** Hold at 1.0 (multiplies/sustains the impact)
- **Decay (D):** Exponential fade (natural release)

**Perfect for:** Arpeggios, plucks, stabs, leads, sequences

---

## Technical Specifications

**Polyphony:** 32 voices  
**Oscillators per voice:** 2× Supersaw layers  
**Max unison per layer:** 32 voices  
**Total oscillators (max):** 2,048 simultaneous  
**Anti-aliasing:** Poly-BLEP  
**Normalization:** Fixed (√32 = 0.177)  
**Output:** Soft clipping (tanh)  

**Envelope:**
- Attack: Quadratic (t²)
- Multiply: Hold at peak (1.0)
- Decay: Exponential (1-t)²

---

## Building

```bash
cargo build --release
cp target/release/libunison_supersaw.so ~/.clap/lap_supersaw.clap
```

Output:
- Linux: `libunison_supersaw.so`
- Windows: `unison_supersaw.dll`
- macOS: `libunison_supersaw.dylib`

---

## License

GPL-3.0-or-later

---

## Credits

**Developer:** lap-plugin
**Framework:** NIH-plug by Robbert van der Helm  
**Version:** 1.0.0  
**Envelope:** AMD (Attack-Multiply-Decay) one-shot design
