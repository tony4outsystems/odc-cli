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

Type "odc list-assets --asset eGov"
Sleep 500ms
Enter
Sleep 3s

Type "odc grant-role eGovPortal eGovPortal tony.lefebvre@outsystems.com"
Sleep 500ms
Enter
Sleep 3s

Type "odc producer-graph eGovPortal"
Sleep 500ms
Enter
Sleep 3s

Type "odc deploy eGovPortal Test"
Sleep 500ms
Enter
Sleep 5s
