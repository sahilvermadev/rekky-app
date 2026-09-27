import json
from pathlib import Path
import subprocess
import tempfile
import unittest

class GazetteerImportTest(unittest.TestCase):
    def test_parent_closure_aliases_and_cycles(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            country=['ZZ','','','', 'Example Country','','','1000','','','','','','','','','1']
            (root/'countryInfo.txt').write_text('\t'.join(country)+'\n')
            (root/'admin1CodesASCII.txt').write_text('ZZ.01\tExample State\tExample State\t2\n')
            (root/'admin2Codes.txt').write_text('ZZ.01.01\tExample District\tExample District\t3\n')
            # City and neighbourhood both have admin parents; city membership is explicit.
            records=[]
            for id,name,feature in [('4','Example City','PPLA'),('5','Example Quarter','PPLX')]:
                records.append('\t'.join([id,name,name,'Old '+name,'0','0','P',feature,'ZZ','','01','01','','','1000','','','Etc/UTC','2026-09-27']))
            (root/'cities500.txt').write_text('\n'.join(records)+'\n')
            (root/'hierarchy.txt').write_text('4\t5\t\n1\t1\tADM\n')
            subprocess.run(['python3',str(Path(__file__).with_name('prepare_geography.py')),str(root)],check=True,capture_output=True)
            rows={r['id']:r for r in map(json.loads,(root/'areas.jsonl').read_text().splitlines())}
            q=rows['geonames:5']
            self.assertEqual(set(q['ancestors']),{'geonames:1','geonames:2','geonames:3','geonames:4'})
            self.assertEqual(q['label'],'Example Quarter, Example City')
            self.assertIn('old example quarter',q['aliases'])
            self.assertEqual(rows['geonames:1']['ancestors'],[])
            self.assertEqual(rows['geonames:4']['label'],'Example City')
            self.assertEqual(json.loads((root/'manifest.json').read_text())['area_count'],5)

if __name__=='__main__': unittest.main()
