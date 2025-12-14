// taken from https://forum.typst.app/t/how-to-auto-size-text-and-images/1290/3
#let fill-height-with-text(min: 0.3em, max: 10em, eps: 0.1em, it) = layout(size => {
  let fits(text-size, it) = {
    // @typstyle off
    measure(width: size.width, { set text(text-size); it }).height <= size.height
  }

  if not fits(min, it) { panic("Content doesn't fit even at minimum text size") }
  if fits(max, it) { return text(max, it) }

  let (a, b) = (min, max)
  while b - a > eps {
    let new = 0.5 * (a + b)
    if fits(new, it) {
      a = new
    } else {
      b = new
    }
  }

  text(a, it)
})

#let label(width: length, height: length, date: text, name: text, code: bytes) = {
  set page(width: width, height: height, margin: 0pt)
  set text(font: "Liberation Sans")

  if code == none {
    // lazy load preview to avoid panicking since the embedded rendering cannot load files
    code = read("dummy_code.svg", encoding: none)
  }

  box(
    inset: 1pt,
    width: width,
    height: height,
  )[
    #place(
      center + top,
      block(height: height * 50%, fill-height-with-text(name)),
      dy: 8pt,
    )
    #place(bottom + center, block(
      height: height * 35%,
      width: width - 20pt,
      [
        #place(horizon + left, image(code, height: 100%))
        #place(horizon + right, block(height: 50%, fill-height-with-text(date)))
      ],
    ))
  ]
}

#label(
  width: sys.inputs.at("width", default: 696) * 1pt,
  height: sys.inputs.at("height", default: 300) * 1pt,
  name: sys.inputs.at("name", default: "Schupfnudel-Wirsing-Auflauf mit Kassler"),
  date: sys.inputs.at("date", default: "12/25"),
  code: sys.inputs.at("code", default: none),
)
