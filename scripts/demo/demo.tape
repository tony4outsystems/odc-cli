# VHS demo tape for ODC CLI
# Run: vhs scripts/demo/demo.tape

Set Shell "bash"
Set FontSize 14
Set Width 1200
Set Height 800
Set Padding 10
Set LetterSpacing 1
Set LineHeight 1.2

# Start demo
Type "odc list-assets --asset eGov"
Enter
Sleep 2s
Type "odc grant-role eGovPortalCore Development eGovPortal tony.lefebvre@outsystems.com"
Enter
Sleep 3s
Type "odc producer-graph eGovPortal"
Enter
Sleep 2s
Type "odc deploy eGovPortal Test"
Enter
Sleep 5s
