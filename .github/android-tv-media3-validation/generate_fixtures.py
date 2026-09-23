from pathlib import Path
import subprocess

out = Path(__file__).resolve().parent / "fixtures"
out.mkdir(parents=True, exist_ok=True)
rates = [
    ("23.976", "24000/1001"),
    ("24", "24"),
    ("25", "25"),
    ("29.97", "30000/1001"),
    ("30", "30"),
    ("50", "50"),
    ("59.94", "60000/1001"),
    ("60", "60"),
]

for label, rate in rates:
    target = out / f"flash-click-{label}.mp4"
    video = (
        f"color=c=black:s=1280x720:r={rate}:d=20,"
        "drawbox=x=0:y=0:w=iw:h=ih:color=white@1:"
        "enable='lt(mod(t,1),0.08)':t=fill"
    )
    audio = "aevalsrc='if(lt(mod(t,1),0.02),0.8*sin(2*PI*1000*t),0)':s=48000:d=20"
    subprocess.run([
        "ffmpeg", "-y", "-hide_banner", "-loglevel", "error",
        "-f", "lavfi", "-i", video,
        "-f", "lavfi", "-i", audio,
        "-c:v", "libx264", "-preset", "veryfast", "-crf", "18",
        "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k",
        "-movflags", "+faststart", "-shortest", str(target)
    ], check=True)
    print(target)
