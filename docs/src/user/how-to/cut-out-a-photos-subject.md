# Cut a photo's subject out

A photo's background traces into shapes of its own, and they sew with the subject: the turf under a dog, a
fence behind it. `--subject` cuts the subject out before the picture is traced, so that only the subject is
sewn. It is off unless you ask for it.

```sh
stitch plan dog.jpg -o dog.pes --subject --cutout dog-cutout.png
```

`stitch` finds the picture's main subject, makes everything else transparent and crops the picture to what
it kept. VectorCraft's command line then traces the cut-out as it traces any picture
([from a picture to a machine file](../tutorials/picture-to-machine-file.md#from-the-command-line)), and
`--size` and the hoop fit the subject, not the whole photo. `stitch export` takes the same options.
`SC-I1206` says which box the subject was found in, how much of the picture was kept and the cut-out's size.
`--cutout` writes the cut-out as a PNG file, so that you can see what was kept before you sew.

[A photo with its background removed](../stitches/automatic.md#a-photo-with-its-background-removed) shows
a cut-out of a dog, its trace and its stitches.

## The models, fetched once

The cut-out runs 3 models on your computer, 49 MB together. `stitch` fetches them the first time you use
`--subject`, and [install](../install.md#the-subject-cut-outs-models) says where it keeps them. Your picture
never leaves your computer. A cut-out takes some 5 seconds for a photo.

## When the cut-out is wrong

Open the file `--cutout` wrote. Every option below takes pixels of the picture as its file stores it,
counted from the left and from the top, and an image viewer shows them under the pointer.

- **Something else was kept with the subject**, such as a tyre behind the dog. Mark a point on it with
  `--not-subject X,Y`.
- **Part of the subject is missing.** Mark a point on the missing part with `--subject-point X,Y`.
- **The wrong thing was taken for the subject**, or `SC-E1202` says none was found. Give the subject's box
  yourself with `--subject-box X0,Y0,X1,Y1`, from its left and top edges to its right and bottom ones.
- Give each kind of point as often as you need.

```sh
stitch plan dog.jpg -o dog.pes --subject --not-subject 640,720 --cutout dog-cutout.png
```

## In VectorCraft

`stitch cutout` writes the cut-out alone, with the same options to correct it:

```sh
stitch cutout dog.jpg -o dog-cutout.png
```

Place `dog-cutout.png` in VectorCraft where you would place the photo, and trace it as the
[tutorial](../tutorials/picture-to-machine-file.md) does. A picture whose background another program made
transparent needs no cut-out: it traces as its subject already.

## What can stop it

| Code | What happened | What to do |
|---|---|---|
| `SC-E1201` | A model couldn't be fetched, or isn't the file expected | The message names each file's address, size and SHA-256, and the folder to put it in |
| `SC-E1202` | No subject was found | Give `--subject-box` |
| `SC-E1203` | The picture isn't a PNG or JPEG file the cut-out reads, or is larger than 16,384 px on a side or 50 megapixels | Save it as PNG or JPEG, or make it smaller |
| `SC-E1207` | This `stitch` was built without the cut-out | Use a `stitch` built with its default features |

Each time, `stitch` writes the report alone, and the picture plans as before without `--subject`.
