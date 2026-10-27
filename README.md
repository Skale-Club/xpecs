# Xpecs

Portable system information, live sensors and benchmarks for Windows, in the spirit of AIDA64. One
small exe: no installer, no driver, no administrator rights. 🇧🇷 [Português](#português) · 🇪🇸
[Español](#español)

Made by **[Skale Club](https://skale.club)**.

## Availability

**Status checked October 26, 2026:** version 0.1.0 is in source control; no GitHub Release or
downloadable executable has been published. Use the [build instructions](docs/DEVELOPMENT.md) until
a release is available. Future binaries belong in
[Releases](https://github.com/Skale-Club/xpecs/releases).

The app can offer signed updates when a newer release contains both an executable and its signature.
Updates require internet access and are installed when you select the update action; your `Data`
folder is retained.

## Run it

Double-click `Xpecs.exe`. It requires Microsoft Edge WebView2 Runtime. Settings (just the language
for now) live in `Data\xpecs.json` next to the exe; reports go to `Data\reports`. Carry the folder
anywhere. Keep the executable in a writable folder. The app creates `Data` beside it.
PortableApps-style metadata is optional; the build and release workflow do not create a PortableApps
package.

Language: the selector at the bottom left (English, Português, Español).

## What it shows

| Page | Contents |
|---|---|
| Summary | Everything at a glance, plus live CPU, memory, GPU and drive readings |
| Processor | Name, cores (performance + efficient), threads, base clock, caches, instruction sets; live usage and clock per thread |
| Motherboard | Board, BIOS version and date, system model |
| Memory | Installed/usable, slots, and every module: size, type, speed, maker, part number, voltage |
| Graphics | GPU, VRAM, drivers, PCIe link, clocks and power limit (NVIDIA via NVML); monitors with model and size |
| Storage | Every drive (interface, type, capacity, firmware, health) with live temperature, read/write speed and activity; volumes with free space |
| Network | Adapters with status, link speed, MAC, IP addresses, gateway, DNS; live traffic |
| Operating system | Edition, version, build, install date, last boot, audio devices |
| Sensors | Available live readings with current, min, max and a 60-second graph |
| Benchmarks | Memory read, write, copy and latency; CPU integer and floating point, single thread and all threads; stress test |

Click any value to copy it. **Export report** saves everything (including a sensor snapshot and
benchmark results) as one HTML file and opens it.

## About temperatures

Without a kernel driver Xpecs cannot generally read CPU package temperature, fans or motherboard
voltages directly. Xpecs does not ship a kernel driver. What you get without a driver:

- GPU temperature, load, clocks, power and fan (NVIDIA)
- Drive temperatures when exposed by the device and Windows storage APIs
- The ACPI thermal zone (on many desktops a fixed board value, not the CPU)

For the rest, open **LibreHardwareMonitor** (free, portable): while it runs, Xpecs reads the
supported sensors it exposes through WMI and lists them on the Sensors page.

## Benchmarks

- Memory read/write: a 512 MiB buffer partitioned among workers. Copy uses   separate 256 MiB source
  and destination buffers. Bandwidth is reported in GB/s.   Latency uses a single random pointer
  chase through 256 MiB and is reported in   nanoseconds. These are Xpecs tests, not directly
  comparable to another tool's scores.
- CPU: about a second of 64-bit integer hashing and FP64 Mandelbrot iterations,   on one thread and
  on all threads. "Scaling" is all threads ÷ one thread.
- Stress test: loads every thread at 100% until you stop it, with live usage,   clocks and
  temperatures beside it.

Close other heavy programs for steadier numbers. Results depend on workload, thermal conditions and
system activity. Sensors vary by device, driver and permissions; an unavailable reading is not a
zero value. LibreHardwareMonitor may need elevation to expose hardware sensors.

## Development

Tauri 2, Rust and plain HTML/CSS/JavaScript. See the [development and release
guide](docs/DEVELOPMENT.md) for prerequisites, clone, build, tests and signed-release instructions.
A push to `main` alone does not publish an executable.

---

## Português

**Disponibilidade:** em 26/10/2026, ainda não há release nem executável publicado. Consulte
[Releases](https://github.com/Skale-Club/xpecs/releases) ou as [instruções de
compilação](docs/DEVELOPMENT.md). As atualizações dependem de uma release assinada e da ação do
usuário.

O Xpecs mostra informações do sistema, sensores ao vivo e benchmarks, no estilo do AIDA64. É um exe
pequeno, sem instalação, sem driver e sem precisar de administrador.

- Abra o `Xpecs.exe`. O idioma fica no canto inferior esquerdo.
- Clique em qualquer valor para copiar. **Exportar relatório** salva tudo num   HTML na pasta
  `Data/reports`, ao lado do executável.
- A temperatura do encapsulamento da CPU, as ventoinhas e as voltagens exigem um   driver. Abra o
  **LibreHardwareMonitor** (gratuito e portátil) e o Xpecs mostra   os sensores disponíveis dele
  automaticamente; algumas leituras podem exigir que ele rode como administrador. Sem ele, leituras
  de GPU NVIDIA e discos dependem do hardware, drivers e permissões disponíveis.
- Benchmarks: leitura, escrita, cópia e latência da memória; CPU inteiro e   ponto flutuante em 1
  thread e em todas; teste de estresse. Os resultados   não são diretamente comparáveis aos de
  ferramentas com outros métodos.

## Español

**Disponibilidad:** al 26/10/2026, aún no hay release ni ejecutable publicado. Consulte
[Releases](https://github.com/Skale-Club/xpecs/releases) o las [instrucciones de
compilación](docs/DEVELOPMENT.md). Las actualizaciones requieren una release firmada y la acción del
usuario.

Xpecs muestra información del sistema, sensores en vivo y benchmarks, al estilo de AIDA64. Es un exe
pequeño, sin instalación, sin controlador y sin necesidad de administrador.

- Abre `Xpecs.exe`. El idioma está en la esquina inferior izquierda.
- Haz clic en cualquier valor para copiarlo. **Exportar informe** guarda todo   en un HTML en
  `Data/reports`, junto al ejecutable.
- La temperatura del encapsulado de la CPU, los ventiladores y los voltajes   necesitan un
  controlador. Abre **LibreHardwareMonitor** (gratuito y portátil)   y Xpecs mostrará los sensores
  disponibles; algunas lecturas pueden requerir que LibreHardwareMonitor se ejecute como
  administrador. Sin él, las lecturas de GPU NVIDIA y discos dependen del hardware, los
  controladores y los permisos disponibles.
- Benchmarks: lectura, escritura, copia y latencia de memoria; CPU entero y   coma flotante en 1
  hilo y en todos; prueba de estrés. Los resultados no son   directamente comparables con
  herramientas que usan otros métodos.

## Skale Club apps

These Windows apps belong to the [Skale-Club organization](https://github.com/Skale-Club). They
share a dark navy interface, blue accents and English, Portuguese and Spanish UI text. Each app has
its own executable and data folder.

| App | Purpose |
|---|---|
| [Xink](https://github.com/Skale-Club/xink) | Draw over the screen during presentations or recordings |
| [Xpecs](https://github.com/Skale-Club/xpecs) | Inspect hardware, live sensors and benchmark results |
| [Xcare](https://github.com/Skale-Club/xcare) | Review PC maintenance, clean selected files and manage startup programs |
| [Xnote](https://github.com/Skale-Club/xnote) | Transcribe meeting audio and turn notes into summaries and tasks |

Xnote currently requires repository access. Report problems through the relevant repository's Issues
tab, including the app version, Windows version and steps to reproduce. Remove personal information
from reports and transcripts before sharing.

---

© 2026 [Skale Club](https://skale.club)
