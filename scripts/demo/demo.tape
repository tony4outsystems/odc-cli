Output demo.gif

Set Shell "bash"
Set FontSize 16
Set Width 1000
Set Height 560
Set Padding 20
Set Theme "Catppuccin Mocha"
Set TypingSpeed 60ms

Hide
Type "clear"
Enter
Sleep 500ms
Show

Type "odc list-apps --search eGov"
Sleep 500ms
Enter
Sleep 3s

Type "odc validate --app eGovPortal --env Sandbox"
Sleep 500ms
Enter
Sleep 4s

Type "odc deploy --app eGovPortal --env Sandbox"
Sleep 500ms
Enter
Sleep 20s
