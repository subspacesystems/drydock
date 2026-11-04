# Drawing

Everything in Drydock begins with a flat shape. This page shows the four drawing tools and the small habits that make them accurate.

## Lines

Press `L` for the Line tool. Click a start point, then click the next point, and each click continues the chain so you can draw several connected edges in a row. Press `Esc` to end the chain or to cancel the line you are drawing. As you move, the cursor snaps to useful places: endpoints, midpoints, centers, points on edges and faces, the ground, and along the red, green and blue axes. A hint beside the cursor names the snap, for example "Endpoint" or "On Red Axis", so you can trust what the next click will attach to.

## Rectangles

Press `R` for the Rectangle tool. Click one corner and then click the opposite corner; Drydock fills in the four edges for you. If you need an exact size, see Typing values below.

## Circles

Press `C` for the Circle tool. Click the center, then click to set the radius. A circle is drawn from segments, and you can decide how many before you draw: type a number followed by `s`, such as `12s`, and the next circle uses that many segments.

## Arcs

Press `A` to draw an arc from a center, a start point and a sweep. Press `Shift+A` for a three-point arc instead; its toolbar button is called 3-Point Arc. The number of segments is set the same way as for a circle, by typing a number and `s`, so you can make an arc as smooth or as chunky as you like.

## Typing values

While you are drawing, you can type a number instead of clicking. Add a unit such as `10cm` or `1/2"` if you like; without a unit, the model's unit is used. Press `Enter` to apply the value. A rectangle takes width and height separated by a comma, like `10cm, 5cm`. The measurements box shows what you typed, and `Backspace` deletes the last character if you make a typo.

## Locking a direction

The arrow keys lock a line to an axis: `Right` is red, `Left` is green, and `Up` or `Down` is blue. Press the same arrow again to release the lock. If you hold `Shift`, the line locks to the direction you are already showing. Locking keeps a line perfectly straight while you choose its length.

## Locking the drawing plane

Shapes are drawn on a plane, normally the face under the cursor. With Rectangle, Circle, Arc and 3-Point Arc, the arrow keys fix that plane before your first click. `Right` draws on a plane facing red, `Left` on one facing green, and `Up` or `Down` on a flat one facing blue. Press the same arrow again to release the lock. Use this to stand a profile up in empty space, for example a circle for a pipe that Follow Me will carry along a path (see [Follow Me](extrude.md)).
