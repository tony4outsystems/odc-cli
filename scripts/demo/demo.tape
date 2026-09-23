Output demo.gif 
Require odc

Set Shell "bash"
#Set TypingSpeed 75ms
Set FontSize 16

Set Width 1200
#Set Height 800
Set Padding 10
#Set LetterSpacing 1
#Set LineHeight 1.2
Set WindowBar Colorful

Type "odc list-assets --asset eGov"
Enter
Sleep 2s
Type "odc grant-role eGovPortalCore Development eGovPortal tony.lefebvre@outsystems.com"
Enter
Sleep 2s
Type "odc producer-graph eGovPortal --output eGovPortal-producers-graph.mmd"
Enter
Sleep 2s
Type "odc deploy eGovPortal Test"
Enter
Sleep 5s
