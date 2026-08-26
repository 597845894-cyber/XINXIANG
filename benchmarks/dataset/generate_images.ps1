#requires -Version 7.0

param(
  [string]$OutputDirectory = (Join-Path $PSScriptRoot 'images')
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
[System.IO.Directory]::CreateDirectory($OutputDirectory) | Out-Null

function New-NoticeScreenshot {
  param(
    [string]$Path,
    [int]$Width,
    [int]$Height,
    [string[]]$Lines
  )

  $bitmap = [System.Drawing.Bitmap]::new($Width, $Height)
  $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
  $graphics.Clear([System.Drawing.Color]::FromArgb(244, 247, 249))
  $graphics.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit

  $titleFont = [System.Drawing.Font]::new('Microsoft YaHei UI', 21, [System.Drawing.FontStyle]::Bold)
  $bodyFont = [System.Drawing.Font]::new('Microsoft YaHei UI', 18, [System.Drawing.FontStyle]::Regular)
  $metaFont = [System.Drawing.Font]::new('Microsoft YaHei UI', 13, [System.Drawing.FontStyle]::Regular)
  $textBrush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(28, 35, 42))
  $metaBrush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(104, 115, 124))
  $bubbleBrush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::White)
  $borderPen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(218, 224, 229), 1)

  $graphics.DrawString('校园通知群（合成样本）', $titleFont, $textBrush, 34, 24)
  $graphics.DrawString('所有姓名、时间与内容均为测试虚构', $metaFont, $metaBrush, 36, 64)
  $bubble = [System.Drawing.Rectangle]::new(28, 96, $Width - 56, $Height - 124)
  $graphics.FillRectangle($bubbleBrush, $bubble)
  $graphics.DrawRectangle($borderPen, $bubble)

  $y = 124
  foreach ($line in $Lines) {
    $graphics.DrawString($line, $bodyFont, $textBrush, 54, $y)
    $y += 44
  }

  $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
  $borderPen.Dispose()
  $bubbleBrush.Dispose()
  $metaBrush.Dispose()
  $textBrush.Dispose()
  $metaFont.Dispose()
  $bodyFont.Dispose()
  $titleFont.Dispose()
  $graphics.Dispose()
  $bitmap.Dispose()
}

New-NoticeScreenshot -Path (Join-Path $OutputDirectory 'clear-relative.png') -Width 920 -Height 420 -Lines @(
  '班级通知',
  '请全体同学明天17:00前完成实验室安全考试。',
  '入口：校园安全学习平台'
)

New-NoticeScreenshot -Path (Join-Path $OutputDirectory 'long-multiple-tasks.png') -Width 920 -Height 820 -Lines @(
  '志愿服务活动安排',
  '面向2025级学生开展社区志愿服务。',
  '第一步：9月5日12:00前在第二课堂平台完成报名。',
  '第二步：9月6日18:00前将安全承诺书PDF发送到活动页面。',
  '第三步：9月8日8:30在东门集合，携带学生证参加活动。',
  '活动自愿参加，完成后可记录志愿时长。'
)
