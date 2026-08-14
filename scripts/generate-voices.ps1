param(
    [Parameter(Mandatory = $true)]
    [string] $OutputDirectory
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Speech
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null

$lines = @(
    @{ File = "m01_01_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = -1; Text = "My brother died because you put your name beneath Teague's lie." },
    @{ File = "m01_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -2; Text = "Yes." },
    @{ File = "m01_03_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = 0; Text = "Do not give me a one-word confession. Give me a reason not to leave you for those riders." },
    @{ File = "m01_04_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -2; Text = "The counter-book proves who Teague bought, who he robbed, and who he means to kill next. I cannot undo Josiah. I can stand between Teague and the living." },
    @{ File = "m01_05_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = 0; Text = "Then survive long enough to testify. I did not drag the truth this far to bury it with you." },
    @{ File = "m01_06_elias.wav"; Voice = "Microsoft David Desktop"; Rate = 0; Text = "Powderburn Company! Hold the creek. Naomi lives. Teague gets nothing else today." },
    @{ File = "m002_01_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = -1; Text = "That courier knows which initials bought my brother's rope." },
    @{ File = "m002_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -2; Text = "Then we take the key, and Teague loses the shelter of numbers." },
    @{ File = "m06_01_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = 0; Text = "We save the station hands first. Paper does not bleed." },
    @{ File = "m06_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -1; Text = "People first. Strongbox second. Teague's men last." },
    @{ File = "m04_01_whitehorse.wav"; Voice = "Microsoft Zira Desktop"; Rate = -2; Text = "Your paper can name the men who profit. My people still pay the price." },
    @{ File = "m04_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -1; Text = "Then today the people leave alive, and the profiteers lose their lie." },
    @{ File = "m07_01_whitehorse.wav"; Voice = "Microsoft Zira Desktop"; Rate = -2; Text = "Do not call this saving us. Save the people in that ravine. Leave the larger words alone." },
    @{ File = "m07_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -2; Text = "No larger words. Show me the ravine." },
    @{ File = "m08_01_doyle.wav"; Voice = "Microsoft David Desktop"; Rate = 1; Text = "The east draw is ugly, narrow, and still our best chance." },
    @{ File = "m08_02_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = 0; Text = "Ugly is a road. Dead is an ending. Take the draw." },
    @{ File = "m02_01_wen.wav"; Voice = "Microsoft David Desktop"; Rate = 0; Text = "They will print the photograph without men like us. Teague means to erase the payroll too." },
    @{ File = "m02_02_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = -1; Text = "Then we keep the workers breathing and make the missing wages impossible to ignore." },
    @{ File = "m09_01_wen.wav"; Voice = "Microsoft David Desktop"; Rate = 0; Text = "They call the grade finished. A road is not finished while the men who built it go hungry." },
    @{ File = "m09_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -2; Text = "Then we collect the wages, not another entry for the dead." },
    @{ File = "m10_01_ames.wav"; Voice = "Microsoft David Desktop"; Rate = 1; Text = "Every red line on this survey crosses somebody's kitchen, grave, or well." },
    @{ File = "m10_02_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = 0; Text = "Then those lines stop being Teague's property tonight." },
    @{ File = "m11_01_wen.wav"; Voice = "Microsoft David Desktop"; Rate = 0; Text = "He died for eleven lines of wire copy. Make them travel farther than he did." },
    @{ File = "m11_02_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = -1; Text = "They reach the train. We remember the man who could not." },
    @{ File = "m13_01_ames.wav"; Voice = "Microsoft David Desktop"; Rate = 1; Text = "I drew clean lines for dirty men. These surveyors should not die correcting my work." },
    @{ File = "m13_02_mercer.wav"; Voice = "Microsoft David Desktop"; Rate = 0; Text = "Then quit confessing and cover the north wall. We bring them home." },
    @{ File = "m14_01_ames.wav"; Voice = "Microsoft David Desktop"; Rate = 0; Text = "The patron is ruined. Men like Teague become most dangerous when money stops protecting them." },
    @{ File = "m14_02_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = 0; Text = "Then we take his protection away and survive what he does next." },
    @{ File = "m12r_01_whitehorse.wav"; Voice = "Microsoft Zira Desktop"; Rate = -2; Text = "We warned them. A warning is not protection." },
    @{ File = "m12r_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -2; Text = "No. Protection is what we do next." },
    @{ File = "m12h_01_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = -1; Text = "The badge got us inside. It also told those prisoners exactly who they had to fear." },
    @{ File = "m12h_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -2; Text = "We free them first. They may judge the disguise after." },
    @{ File = "m003_01_whitehorse.wav"; Voice = "Microsoft Zira Desktop"; Rate = -2; Text = "Do not mistake fighting Teague for owning what happens here." },
    @{ File = "m003_02_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = 0; Text = "We own only our choices. Teague's killers are one of them." },
    @{ File = "m05_01_doyle.wav"; Voice = "Microsoft David Desktop"; Rate = 1; Text = "I have tracked animals with kinder records than this." },
    @{ File = "m05_02_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = 0; Text = "Take the names. Burn the prices. People are not Teague's inventory." },
    @{ File = "m15_01_whitehorse.wav"; Voice = "Microsoft Zira Desktop"; Rate = -2; Text = "Every crate you stop is one less payment for somebody else's suffering." },
    @{ File = "m15_02_doyle.wav"; Voice = "Microsoft David Desktop"; Rate = 1; Text = "Convoy is four wagons, six rifles, and no innocence. I can work with that." },
    @{ File = "m16_01_whitehorse.wav"; Voice = "Microsoft Zira Desktop"; Rate = -2; Text = "Four days late can be the distance between warning and mourning." },
    @{ File = "m16_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -2; Text = "We cannot steal those days back. We can keep Teague from stealing the truth." },
    @{ File = "m18_01_ames.wav"; Voice = "Microsoft David Desktop"; Rate = 0; Text = "Teague does not care which side shoots first. He owns the story after." },
    @{ File = "m18_02_doyle.wav"; Voice = "Microsoft David Desktop"; Rate = 1; Text = "Then we remove his storytellers before they pull a trigger." },
    @{ File = "m17_01_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = -1; Text = "This is why I carried the names. Not to live inside grief. To make room for this." },
    @{ File = "m17_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -2; Text = "Then this ground, these homes, and your future are the mission." },
    @{ File = "m20_01_ruelas.wav"; Voice = "Microsoft David Desktop"; Rate = 0; Text = "My family gathered salt before their courthouse had a roof. Teague calls memory trespass." },
    @{ File = "m20_02_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = 0; Text = "Then we defend the people who remember, not just the paper that agrees." },
    @{ File = "m19_01_whitehorse.wav"; Voice = "Microsoft Zira Desktop"; Rate = -2; Text = "A name is not evidence for your cause. It belongs first to the person who answers to it." },
    @{ File = "m19_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -2; Text = "You carry the list. You decide where it goes." },
    @{ File = "m21_01_alcantara.wav"; Voice = "Microsoft David Desktop"; Rate = 0; Text = "The fever does not choose victims. Teague does, even here." },
    @{ File = "m21_02_naomi.wav"; Voice = "Microsoft Zira Desktop"; Rate = 0; Text = "Then we take back the medicine and leave him nobody to count as profit." },
    @{ File = "m24_01_teague.wav"; Voice = "Microsoft David Desktop"; Rate = 1; Text = "Ward, you built a religion from a bookkeeping error. Come upstairs and learn what the country actually costs." },
    @{ File = "m24_02_elias.wav"; Voice = "Microsoft David Desktop"; Rate = -2; Text = "No sermon. No absolution. We bring you out alive, and the people you priced speak for themselves." }
)

foreach ($line in $lines) {
    $synth = New-Object System.Speech.Synthesis.SpeechSynthesizer
    try {
        $synth.SelectVoice($line.Voice)
        $synth.Rate = $line.Rate
        $path = Join-Path $OutputDirectory $line.File
        $synth.SetOutputToWaveFile($path)
        $synth.Speak($line.Text)
    }
    finally {
        $synth.Dispose()
    }
}

Write-Output "Generated $($lines.Count) offline dialogue tracks in $OutputDirectory"
