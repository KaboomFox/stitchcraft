# Reading the before and after sheet

Every pull request from a branch of the repository has a sheet. It is a page that shows what the change
does to a set of standard pictures, and a reader doesn't have to build anything. The preview's comment on the pull request links to
it under **Before and after** and lists the numbers that got worse most.

## The table and the pictures

The table has a row for each picture and a column for each number. A cell with one number did not change.
A cell that changed shows main's number, an arrow and the pull request's number. It is red, with "worse",
when more of the number is worse and it rose, and green, with "better", when it fell. A change in the
stitches is not marked, because more stitches are neither better nor worse.

A picture that does not plan has the reason in its row. It is marked when it planned on main.

Under the table come the pictures, main's on the left and the pull request's on the right, each drawn as
the machine would sew it. Look here for what no number says, such as a shape that changed its stitch.

The part headed **In the browser** says what the editor showed in Chromium, and has a picture of the
editor with a traced photo's stitches drawn, for main and for the pull request.

## Main's numbers from another commit

The page names the commit of main its left side is from. When that commit and the branch differ by more
than the branch's own commits, a line under the heading says how, and some marked numbers may be another
change's. Merge main into the branch, or wait for main's run to end, and push again.

## A run that fails

A number that got worse does not fail the run. Decide whether the change means it, and say so in the pull
request. The run fails when the editor and `stitch` write different files of the same document, or when
the Auto effect's time is above its limit. The job's log names the first byte that differs, and its
`sheet-work` artifact has both files. [The design page](../design/before-after-sheet.md) says how the
sheet is made.
