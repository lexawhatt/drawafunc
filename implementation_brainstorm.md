# Drawafunc Implementation Brainstorm

## Цель

Нужно лучше передавать намерение пользователя: не просто повторять каждую
дрожащую точку мыши, а понимать, где пользователь хотел прямую линию, где
плавную кривую, где острый угол, где замкнутый контур, а где случайный шум.

Текущий MVP делает так:

1. Пользователь рисует stroke.
2. Точки немного сглаживаются.
3. Ramer-Douglas-Peucker упрощает polyline.
4. Соседние точки превращаются в линейные сегменты.
5. Desmos получает batched parametric expression через списки.

Это работает, но это все еще "рисунок как много прямых отрезков", а не
"рисунок как математическая форма".

## Главная проблема

Есть конфликт между тремя целями:

- accuracy: результат должен быть похож на исходный рисунок;
- simplicity: выражение должно быть коротким и быстрым для Desmos;
- intent: результат должен отражать то, что пользователь хотел нарисовать, а не
  шум руки/мыши.

Нельзя всегда максимизировать accuracy. Если повторять каждую микродрожь, мы
получим тяжелый и некрасивый результат. Нужно дать пользователю понятные режимы
качества и внутри каждого режима использовать разные параметры алгоритмов.

## Quality presets

### Rough

Для быстрых черновиков и тяжелых рисунков.

Поведение:

- сильное упрощение;
- агрессивное сглаживание;
- меньше сегментов;
- хуже повторяет мелкие детали;
- хорошо подходит для силуэтов и крупных контуров.

Примерные настройки:

- point merge distance: high;
- smoothing strength: high;
- simplification tolerance: high;
- Bezier max error: high;
- corner preservation: medium;
- max segments: low.

### Default

Основной режим.

Поведение:

- сохраняет общую форму;
- убирает дрожание;
- не слишком раздувает Desmos output;
- старается сохранять углы, если они визуально важны.

Примерные настройки:

- point merge distance: medium;
- smoothing strength: medium;
- simplification tolerance: medium;
- Bezier max error: medium;
- corner preservation: high;
- max segments: medium.

### Smooth

Для красивых плавных кривых.

Поведение:

- предпочитает плавность;
- дуги и волны должны становиться Bezier/spline curves;
- мелкие углы могут сглаживаться;
- хорошо подходит для букв, сердечек, органических контуров.

Примерные настройки:

- point merge distance: medium;
- smoothing strength: high;
- simplification tolerance: low/medium;
- Bezier max error: medium;
- corner preservation: low/medium;
- max segments: medium.

### Precise

Для случаев, где важно повторить рисунок максимально близко.

Поведение:

- меньше сглаживания;
- меньше упрощения;
- больше сегментов;
- тяжелее для Desmos;
- полезно для сложных деталей.

Примерные настройки:

- point merge distance: low;
- smoothing strength: low;
- simplification tolerance: low;
- Bezier max error: low;
- corner preservation: high;
- max segments: high.

## Настройки в UI

Минимальный набор:

- Quality: Rough | Default | Smooth | Precise;
- Output: Auto | Lines | Bezier | Mixed;
- Preserve corners: checkbox;
- Smoothness: slider;
- Max complexity: slider;
- Close path: Auto | On | Off;
- Simplify: existing slider, but лучше привязать его к preset.

Важно: пользователь не должен видеть 20 математических параметров сразу. Нужно
дать presets, а advanced-настройки спрятать под раскрываемый блок.

## Premade shapes, text, emoji

Это отдельное направление, не замена freehand drawing.

Freehand нужен, когда пользователь хочет нарисовать что-то свое. Premade
elements нужны, когда пользователь хочет быстро собрать композицию: открытку,
надпись, символ, мем, математический рисунок.

Пример сценария:

1. Пользователь выбирает Text.
2. Пишет `Даша, ты прекрасна` или другой текст.
3. Добавляет heart/star/sparkle.
4. Двигает, масштабирует, поворачивает элементы.
5. Нажимает Generate.
6. Получает Desmos output, который можно отправить как "математическую открытку".

### Почему это важно

- резко снижает friction;
- дает красивый результат без ручного рисования;
- делает приложение полезным не только для художников;
- позволяет использовать clean primitives вместо noisy strokes;
- часто используемые формы можно экспортировать намного компактнее.

### Premade primitives

Минимальный набор:

- circle;
- ellipse;
- heart;
- star;
- arrow;
- spiral;
- sine wave;
- flower;
- bracket/brace;
- speech bubble;
- common emoji-like symbols.

Каждый primitive должен иметь:

- normalized local coordinate system;
- editable transform: position, scale, rotation;
- style metadata: color, thickness later;
- export strategy;
- preview renderer.

### Text editor

Текст лучше делать не через распознавание нарисованных букв, а как отдельный
object type.

MVP path:

1. Text object stores string, font size, position, alignment.
2. Preview renders text normally in app.
3. Generate converts text to vector outlines.
4. Outlines go through same Bezier/line export pipeline.

Implementation options:

- use font outline extraction crate;
- ship a simple built-in single-line vector font;
- use Hershey fonts for line-based text;
- support only ASCII first, then add Cyrillic through a real font pipeline.

Important edge cases:

- Cyrillic;
- spaces;
- multiline text;
- font availability;
- emoji color fonts;
- very small text;
- huge text that explodes Desmos complexity.

### Emoji-like editor

Full Unicode emoji rendering is hard because modern emoji are colored glyphs,
often not simple vector outlines.

Better approach:

- do not promise full system emoji support first;
- provide built-in emoji-like vector symbols;
- map common emoji requests to internal primitives.

Examples:

- heart -> built-in heart curve;
- star -> built-in star polygon/Bezier;
- sparkle -> small four-point star;
- smile -> circle + arcs;
- flower -> repeated petals;
- arrow -> line + triangle head.

This gives predictable Desmos export. Real font emoji can come later as an
import/vectorization feature.

### Formula-first primitives

Some premade shapes should not be converted from points. They already have clean
math.

Examples:

- circle: `(cx+r cos(t), cy+r sin(t))`;
- ellipse: `(cx+a cos(t), cy+b sin(t))`;
- heart: known parametric heart equation or Bezier outline;
- star: polygon or parametric-ish line batch;
- sine wave: `y=a sin(kx+p)+b` over interval;
- spiral: `(a t cos(t), a t sin(t))`.

This is better than drawing these shapes as sampled polylines because:

- fewer expressions;
- cleaner result;
- editable parameters;
- better semantics.

### Object model implication

Current model stores only strokes. We need object-level scene data.

Possible model:

```rust
enum SceneObject {
    Stroke(StrokeObject),
    Text(TextObject),
    Premade(PremadeObject),
}

struct Transform2D {
    position: Point,
    scale: Point,
    rotation: f32,
}

enum PremadeKind {
    Circle,
    Ellipse,
    Heart,
    Star,
    Arrow,
    Spiral,
    Sparkle,
}
```

Generation should operate on `SceneObject`, not only on raw strokes.

## Pipeline v2

### 1. Raw stroke capture

Сохранять больше данных:

- points;
- timestamp per point;
- pointer pressure, если доступно;
- pointer velocity;
- original screen scale;
- stroke tool;
- intended closed/open hint.

Даже если часть данных пока не используется, она пригодится для будущих
алгоритмов.

### 2. Point cleanup

До любого fitting:

- remove duplicate points;
- remove points closer than minimum distance;
- remove tiny hooks at stroke start/end;
- optionally resample by arc length;
- normalize direction if needed.

Resampling важен: если пользователь медленно рисует один участок, там будет
слишком много точек. Если быстро проводит другой, точек будет мало. Алгоритмы
должны видеть форму, а не скорость руки.

### 3. Noise reduction

Варианты:

- moving average;
- gaussian-like smoothing;
- Chaikin smoothing;
- Savitzky-Golay smoothing;
- Kalman/One Euro filter для live input.

Не надо всегда сглаживать одинаково. Сглаживание должно зависеть от режима
качества и от углов. Если участок похож на угол, его нельзя размазывать как
обычную кривую.

### 4. Corner detection

Нужно определить важные углы до Bezier fitting.

Признаки угла:

- резкая смена направления;
- локальный максимум curvature;
- низкая скорость указателя около точки;
- пользователь сделал pause;
- точка близко к самопересечению;
- маленький радиус поворота.

После detection stroke делится на spans:

- smooth span;
- straight span;
- corner point;
- ambiguous span.

Это критично. Без corner detection Bezier fitting будет либо сглаживать углы,
либо плодить много кривых вокруг них.

### 5. Segment classification

Каждый span можно классифицировать:

- line;
- circular arc;
- cubic Bezier;
- spline segment;
- noisy/unknown polyline.

Simple shape recognition нужно делать до общего fitting:

- почти прямая линия -> line;
- почти круг/эллипс -> analytic circle/ellipse или parametric ellipse;
- плавная дуга -> Bezier или arc;
- closed contour -> closed spline/Fourier later.

### 6. Fitting

MVP+ путь:

1. Split stroke by corners.
2. For each span, try line fit.
3. If line error is low, export line segment.
4. Otherwise fit cubic Bezier.
5. If cubic error too high, split span and retry recursively.
6. Fall back to polyline if fitting fails.

Для Bezier fitting можно использовать Schneider's algorithm:

- estimate endpoint tangents;
- chord-length parameterization;
- solve control points by least squares;
- compute max error;
- split at max error;
- recurse.

Это классический подход из "An Algorithm for Automatically Fitting Digitized
Curves" by Philip J. Schneider.

## Bezier export to Desmos

Cubic Bezier:

```text
B(t) = (1-t)^3 P0 + 3(1-t)^2 t P1 + 3(1-t)t^2 P2 + t^3 P3
```

Для Desmos batched lists:

```text
X_0=[...]
X_1=[...]
X_2=[...]
X_3=[...]
Y_0=[...]
Y_1=[...]
Y_2=[...]
Y_3=[...]
((1-t)^3X_0+3(1-t)^2tX_1+3(1-t)t^2X_2+t^3X_3,
 (1-t)^3Y_0+3(1-t)^2tY_1+3(1-t)t^2Y_2+t^3Y_3)
```

Плюсы:

- одна строка может рисовать много Bezier segments;
- меньше сегментов для плавных кривых;
- лучше визуальное качество;
- Desmos output остается copyable.

Минусы:

- формула длиннее;
- нужно хранить control points;
- нужно уметь превьюить Bezier внутри приложения.

## Mixed output

Лучший долгосрочный вариант: mixed representation.

Рисунок разбивается на primitives:

- line batch;
- cubic Bezier batch;
- ellipse batch;
- point/noise fallback.

Desmos export тогда может быть:

```text
# line lists
LX_1=[...]
LX_2=[...]
LY_1=[...]
LY_2=[...]
(LX_1+(LX_2-LX_1)t,LY_1+(LY_2-LY_1)t)

# bezier lists
BX_0=[...]
BX_1=[...]
BX_2=[...]
BX_3=[...]
...
```

Да, это несколько выражений, но не сотни. И каждый expression отвечает за свой
тип геометрии.

## Error metrics

Нужно считать ошибку не только в одну сторону.

Метрики:

- max distance from original points to fitted curve;
- average distance;
- Hausdorff-like distance;
- angle/corner preservation score;
- segment count;
- Desmos expression length;
- estimated Desmos cost.

Preview должен показывать:

- green: good fit;
- yellow: visible deviation;
- red: bad fit;
- complexity score.

## Intent heuristics

### Пользователь хотел прямую

Признаки:

- low perpendicular error to line;
- direction stable;
- stroke speed relatively high;
- no important curvature peaks.

Действие:

- snap to line, если error ниже threshold;
- показывать preview, не менять original stroke.

### Пользователь хотел угол

Признаки:

- direction change above threshold;
- local speed drop;
- cluster of points near vertex.

Действие:

- split at corner;
- preserve exact corner point;
- do not smooth across corner.

### Пользователь хотел замкнутую форму

Признаки:

- distance between first and last point small relative to bbox;
- stroke end direction points toward start;
- closed path setting Auto.

Действие:

- optionally close path;
- preserve closure in fitting;
- preview closure before export.

### Пользователь случайно оставил хвост

Признаки:

- very short final segment;
- sharp tiny reversal;
- end hook much smaller than bbox.

Действие:

- trim hook in Rough/Default/Smooth;
- preserve in Precise.

## Data model changes

Добавить:

```rust
enum QualityPreset {
    Rough,
    Default,
    Smooth,
    Precise,
}

enum OutputMode {
    Auto,
    Lines,
    Bezier,
    Mixed,
}

struct GenerationSettings {
    quality: QualityPreset,
    output_mode: OutputMode,
    preserve_corners: bool,
    smoothness: f32,
    max_complexity: usize,
    close_path: ClosePathMode,
}

enum GeneratedPrimitive {
    Line { from: Point, to: Point },
    CubicBezier { p0: Point, p1: Point, p2: Point, p3: Point },
}
```

Важно: `generated_preview` лучше хранить не как `Vec<Vec<Point>>`, а как
структурированный `GeneratedScene`, чтобы preview/export могли использовать один
источник истины.

## Proposed module structure

Текущие модули уже дают нормальную основу:

- `model.rs`: raw project data;
- `geometry.rs`: cleanup/simplification primitives;
- `desmos.rs`: export only;
- `app.rs`: workflow;
- `ui.rs`: egui rendering/input;
- `persistence.rs`: project JSON.

Следующие модули:

- `generation.rs`: orchestration of pipeline;
- `settings.rs`: presets and advanced parameters;
- `primitives.rs`: Line/Bezier/Ellipse generated geometry;
- `fitting/line.rs`;
- `fitting/bezier.rs`;
- `fitting/corner.rs`;
- `preview.rs`: render generated primitives, error heatmap later.

## Implementation phases

### Phase 1: Quality presets for current line pipeline

No Bezier yet.

Tasks:

- add `GenerationSettings`;
- add `QualityPreset`;
- map presets to simplification/smoothing values;
- move generation from `app.rs` to `generation.rs`;
- keep current Desmos batched line export;
- update UI with Quality segmented/radio control.

Benefit:

- user gets control immediately;
- code gets ready for fitting pipeline.

### Phase 2: Better preprocessing

Tasks:

- resample by arc length;
- remove tiny hooks;
- improve smoothing to avoid smoothing through corners;
- add basic corner detection;
- show detected corners in preview optionally.

Benefit:

- current line output already improves;
- Bezier fitting becomes easier.

### Phase 3: Cubic Bezier fitting

Tasks:

- implement or use crate for Bezier fitting;
- add `GeneratedPrimitive::CubicBezier`;
- add Bezier preview rendering;
- add batched Bezier Desmos export;
- add Mixed output mode.

Benefit:

- smooth curves need fewer primitives;
- Desmos result becomes more mathematically meaningful.

### Phase 4: Shape recognition

Tasks:

- line recognition;
- circle/ellipse recognition;
- rectangle/polygon recognition;
- closed path mode;
- object-level editing.

Benefit:

- simple intentional shapes become clean formulas;
- less dependence on freehand noise.

### Phase 5: Compose mode

Tasks:

- add `SceneObject` model;
- add premade shape picker;
- add transform handles for move/scale/rotate;
- add basic text object;
- add built-in vector symbols: heart, star, sparkle, arrow;
- export formula-first primitives directly where possible;
- route text/vector symbols through Bezier or line export.

Benefit:

- users can create useful compositions without drawing everything manually;
- common shapes become cleaner and cheaper than freehand approximations;
- product becomes closer to a mathematical card/poster editor, not just a
  tracing tool.

## Important tradeoffs

### Accuracy vs intention

A perfect trace of hand jitter is technically accurate but product-wise wrong.
Default should favor intention over raw accuracy.

### One expression vs readable output

Batched lists are fast and compact but not very readable. A future export setting
could offer:

- Compact for Desmos performance;
- Readable for learning/debugging;
- Debug with comments and per-primitive expressions.

### Automatic vs manual correction

Full automation will fail sometimes. The product should expose lightweight fixes:

- mark point as corner;
- close/open path;
- simplify more/less;
- convert selected span to line;
- split stroke.

## Near-term recommendation

Do this next:

1. Add `GenerationSettings` and `QualityPreset`.
2. Move generation out of `app.rs`.
3. Make Rough/Default/Smooth/Precise affect smoothing and simplification.
4. Keep batched line Desmos export as the stable output.
5. Add object-level `SceneObject` before text/premade shapes.
6. Then implement Bezier fitting behind `OutputMode::Bezier` or `Mixed`.

This gives immediate user-facing value while setting up the architecture for
real curve fitting and compose mode.
