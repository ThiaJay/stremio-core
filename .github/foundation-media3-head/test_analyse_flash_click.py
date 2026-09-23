from pathlib import Path
import json
import subprocess
import tempfile

ANALYSER = Path(__file__).resolve().with_name('analyse_flash_click.py')

with tempfile.TemporaryDirectory() as td_value:
    td = Path(td_value)
    recording = td / 'synthetic-80ms.mp4'
    result_path = td / 'result.json'
    video = (
        "color=c=black:s=640x360:r=120:d=8,"
        "drawbox=x=0:y=0:w=iw:h=ih:color=white@1:enable='lt(mod(t,1),0.08)':t=fill"
    )
    audio = "aevalsrc='if(lt(mod(t,1),0.02),0.8*sin(2*PI*1000*t),0)':s=48000:d=8"
    subprocess.run([
        'ffmpeg','-y','-hide_banner','-loglevel','error',
        '-f','lavfi','-i',video,
        '-f','lavfi','-i',audio,
        '-filter_complex','[1:a]adelay=80[a]',
        '-map','0:v','-map','[a]',
        '-c:v','libx264','-preset','veryfast','-crf','18','-pix_fmt','yuv420p',
        '-c:a','aac','-b:a','192k','-t','8',str(recording)
    ], check=True)
    subprocess.run(['python3',str(ANALYSER),str(recording),'--fps','120','--out',str(result_path)], check=True)
    result=json.loads(result_path.read_text())
    assert result['schemaVersion'] == 2
    assert result['pairs'] >= 6, result
    assert 60 <= result['medianOffsetMs'] <= 100, result
    assert abs(result['fittedProgressiveDriftMs']) <= 25, result
    assert 115 <= result['measuredCameraFps'] <= 125, result
    assert result['acceptanceInferred'] is False
    print('synthetic analyser validation passed', json.dumps(result, sort_keys=True))
